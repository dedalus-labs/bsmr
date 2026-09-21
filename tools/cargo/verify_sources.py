# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Acquires one locked crate, then verifies frozen archive and manifest checks.

import hashlib
import json
import shutil
import socket
import tempfile
from pathlib import Path
import subprocess
import sys


def main() -> None:
    """Corruption in an isolated acquired source must never produce a graph."""
    root = Path(__file__).resolve().parent / "target/source-verification"
    root.mkdir(parents=True, exist_ok=True)
    binary, toolchain = Path(sys.argv[1]).resolve(), Path(sys.argv[2]).resolve()
    workspace, cargo_home = root / "workspace", root / "cargo-home"
    (workspace / "src").mkdir(parents=True, exist_ok=True)
    if cargo_home.exists():
        shutil.rmtree(cargo_home)
    config = workspace / ".cargo/config.toml"
    config.parent.mkdir(exist_ok=True)
    config.write_text("[net]\nretry=0\n")
    (workspace / "Cargo.toml").write_text('[package]\nname="source-verification"\nversion="0.1.0"\nedition="2024"\n[dependencies]\nitoa="=1.0.18"\n[workspace]\n')
    (workspace / "src/lib.rs").write_text('compile_error!("PLANNING_MUST_NOT_COMPILE");\n')
    env = {"PATH": f"{toolchain}:/usr/bin:/bin", "CARGO_HOME": str(cargo_home), "RUSTC": str(toolchain / "rustc")}
    subprocess.run([toolchain / "cargo", "generate-lockfile"], cwd=workspace, env=env, check=True)
    request = {"manifest": str(workspace / "Cargo.toml"), "package": "source-verification",
               "mode": "build", "target_filter": {"kind": "package"}, "source_policy": "offline", "features": [], "default_features": True, "all_features": False,
               "target": None, "profile": "dev", "cargo_home": str(cargo_home),
               "rustc": str(toolchain / "rustc"), "target_directory": str(root / "planner-target")}
    lock = (workspace / "Cargo.lock").read_bytes()

    def plan(case: str) -> subprocess.CompletedProcess[str]:
        """Record a frozen plan against the currently prepared source bytes."""
        before = (workspace / "Cargo.lock").read_bytes()
        manifest_before = (workspace / "Cargo.toml").read_bytes()
        output = subprocess.run([binary], input=json.dumps(request), env=env, text=True, capture_output=True)
        assert (workspace / "Cargo.lock").read_bytes() == before
        assert (workspace / "Cargo.toml").read_bytes() == manifest_before
        (root / f"{case}.json").write_text(output.stdout)
        (root / f"{case}.stderr").write_text(output.stderr)
        return output

    result = plan("cold-offline")
    assert result.returncode != 0, "offline planning accepted unacquired sources"
    request["source_policy"] = "acquire-locked"
    result = plan("valid")
    assert result.returncode == 0, result.stderr
    graph = json.loads(result.stdout)
    request["source_policy"] = "offline"
    replay = plan("frozen-reuse")
    assert replay.returncode == 0 and json.loads(replay.stdout) == graph, replay.stderr
    source = next(unit["source"] for unit in graph["units"] if unit["package_name"] == "itoa")
    archive, manifest = Path(source["archive"]["path"]), Path(source["manifest"])
    archive_bytes, manifest_bytes = archive.read_bytes(), manifest.read_bytes()
    assert source["archive"]["size"] == len(archive_bytes)
    assert source["checksum"] == hashlib.sha256(archive_bytes).hexdigest()
    try:
        manifest.write_bytes(manifest_bytes + b'\n[package.metadata]\nplanner_corruption=true\n')
        result = plan("manifest-corrupt")
        assert result.returncode != 0 and "cached manifest differs" in result.stderr, result.stderr
        manifest.write_bytes(manifest_bytes)
        archive.write_bytes(archive_bytes[:-1] + bytes([archive_bytes[-1] ^ 1]))
        request["source_policy"] = "acquire-locked"
        result = plan("archive-corrupt")
        assert result.returncode != 0 and "checksum mismatch" in result.stderr, result.stderr
        request["source_policy"] = "offline"
        archive.unlink()
        result = plan("archive-missing")
        assert result.returncode != 0, "accepted missing acquired archive"
    finally:
        archive.write_bytes(archive_bytes)
        manifest.write_bytes(manifest_bytes)
    request["source_policy"] = "acquire-locked"
    (workspace / "Cargo.lock").write_bytes(lock.replace(source["checksum"].encode(), b"0" * 64))
    result = plan("locked-checksum-mismatch")
    assert result.returncode != 0 and "checksum" in result.stderr.lower(), result.stderr
    (workspace / "Cargo.lock").write_bytes(lock)
    saved_source = root / "saved-source"
    manifest.parent.rename(saved_source)
    archive.unlink()
    try:
        with socket.socket() as unreachable:
            unreachable.bind(("127.0.0.1", 0))
            config.write_text(f'[net]\nretry=0\n[http]\nproxy="http://127.0.0.1:{unreachable.getsockname()[1]}"\ntimeout=1\n')
            result = plan("network-failure")
            assert result.returncode != 0 and "failed to download" in result.stderr.lower(), result.stderr
    finally:
        config.write_text("[net]\nretry=0\n")
        saved_source.rename(manifest.parent)
        archive.write_bytes(archive_bytes)
    verify_git(binary, toolchain, root, env)
    marker = root / "executed"
    marker.unlink(missing_ok=True)
    hook = root / "compiler-hook"
    hook.write_text(f"#!/bin/sh\ntouch {marker}\nexit 1\n")
    hook.chmod(0o700)
    request["rustc"] = str(hook)
    env["RUSTC"] = str(hook)
    for name, contents in [
        ("credential-provider", f'[registry]\nglobal-credential-providers=["{hook}"]\n'),
        ("named-credential-provider", f'[registries.test]\ncredential-provider="{hook}"\n'),
        ("git-cli", '[net]\ngit-fetch-with-cli=true\n'),
    ]:
        config.write_text(contents)
        result = plan(f"reject-{name}")
        assert result.returncode != 0 and "acquisition configuration" in result.stderr, result.stderr
        assert not marker.exists(), name
    config.write_text("[net]\nretry=0\n")
    assert (workspace / "Cargo.lock").read_bytes() == lock
    summary = {"locked_acquisition": True, "locked_git_acquisition": True, "git_package_and_workspace_manifests_verified": True, "frozen_reuse": True, "network_failure_rejected": True,
               "command_hooks_rejected_before_probe": True, "registry_archive_verified": True, "cached_manifest_verified": True,
               "corrupt_or_missing_sources_rejected": True, "lock_unchanged": True}
    (root / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    print(json.dumps(summary))


def verify_git(binary: Path, toolchain: Path, root: Path, env: dict) -> None:
    """A locked Git dependency downloads once and then plans with its origin absent."""
    workspace = root / "git-workspace"
    (workspace / "src").mkdir(parents=True, exist_ok=True)
    (workspace / "src/lib.rs").write_text('compile_error!("PLANNING_MUST_NOT_COMPILE");\n')
    with tempfile.TemporaryDirectory(dir=root) as origin:
        repository = Path(origin)
        (repository / "crates/member/src").mkdir(parents=True)
        (repository / "Cargo.toml").write_text('[workspace]\nmembers=["crates/member"]\nresolver="2"\n[workspace.package]\nversion="0.1.0"\nedition="2024"\n')
        (repository / "crates/member/Cargo.toml").write_text('[package]\nname="git-dependency"\nversion.workspace=true\nedition.workspace=true\n')
        (repository / "crates/member/src/lib.rs").write_text('compile_error!("GIT_SOURCE_MUST_NOT_COMPILE");\n')
        git_env = env | {"GIT_CONFIG_GLOBAL": "/dev/null", "GIT_CONFIG_NOSYSTEM": "1"}
        for args in [["init", "--quiet"], ["add", "."],
                     ["-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "-c", "commit.gpgsign=false", "commit", "--quiet", "-m", "fixture"]]:
            subprocess.run(["git", *args], cwd=repository, env=git_env, check=True)
        revision = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=repository, env=git_env, text=True).strip()
        manifest = workspace / "Cargo.toml"
        manifest.write_text('[package]\nname="git-verification"\nversion="0.1.0"\nedition="2024"\n[workspace]\n'
                            f'[dependencies]\ngit-dependency={{git="{repository.as_uri()}",rev="{revision}"}}\n')
        subprocess.run([toolchain / "cargo", "generate-lockfile"], cwd=workspace, env=git_env, check=True)
        shutil.rmtree(Path(env["CARGO_HOME"]) / "git")
        lock = (workspace / "Cargo.lock").read_bytes()
        request = {"manifest": str(manifest), "package": "git-verification", "mode": "build",
                   "target_filter": {"kind": "library"}, "source_policy": "acquire-locked",
                   "features": [], "default_features": True, "all_features": False,
                   "target": None, "profile": "dev", "cargo_home": env["CARGO_HOME"],
                   "rustc": env["RUSTC"], "target_directory": str(root / "git-target")}
        acquired = subprocess.run([binary], input=json.dumps(request), env=env, text=True, capture_output=True, check=True)
        graph = json.loads(acquired.stdout)
        source = next(unit["source"] for unit in graph["units"] if unit["package_name"] == "git-dependency")
        assert source["git_revision"] == revision
    request["source_policy"] = "offline"
    replay = subprocess.run([binary], input=json.dumps(request), env=env, text=True, capture_output=True, check=True)
    assert json.loads(replay.stdout) == graph
    for kind, manifest in [("package", Path(source["manifest"])), ("workspace", Path(source["root"]).parents[1] / "Cargo.toml")]:
        contents = manifest.read_bytes()
        try:
            manifest.write_bytes(contents + b"\n# changed cached manifest\n")
            rejected = subprocess.run([binary], input=json.dumps(request), env=env, text=True, capture_output=True)
            (root / f"git-{kind}-manifest-corrupt.stderr").write_text(rejected.stderr)
            assert rejected.returncode != 0 and "cached Git manifest differs" in rejected.stderr, rejected.stderr
        finally:
            manifest.write_bytes(contents)
    assert (workspace / "Cargo.lock").read_bytes() == lock
    (root / "git-acquired.json").write_text(acquired.stdout)


if __name__ == "__main__":
    main()
