# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Verifies configured Cargo units against the pinned Cargo CLI.

import hashlib
import fcntl
import json
import re
from pathlib import Path
import subprocess
import sys
import tempfile


ROOT = Path(__file__).resolve().parent
FIXTURE = ROOT / "fixture"
RECEIPTS = ROOT / "receipts"


def write_fixture() -> None:
    """Create a resolver-2 workspace with target, host, macro and dev features."""
    files = {
        "Cargo.toml": '''[workspace]
members = ["app", "shared", "derive", "foreign"]
resolver = "2"
[profile.dev]
opt-level = 1
[profile.dev.build-override]
opt-level = 0
[profile.release]
opt-level = 3
debug = 1
panic = "abort"
[profile.release.package.shared]
opt-level = 2
[profile.release.build-override]
opt-level = 0
''',
        ".cargo/config.toml": '''[target.aarch64-apple-darwin]
rustflags = ["--cfg", "target_marker"]
rustdocflags = ["--cfg", "doc_marker"]
''',
        "app/Cargo.toml": '''[package]
name = "app"
version = "0.1.0"
edition = "2024"
authors = ["First Author", "Second Author"]
description = "Planner environment fixture"
license = "MIT"
rust-version = "1.90"
[features]
extra = []
[[bin]]
name = "gated"
path = "src/gated.rs"
required-features = ["extra"]
[dependencies]
shared = { path = "../shared", features = ["target"] }
derive = { path = "../derive" }
[build-dependencies]
shared = { path = "../shared", features = ["host"] }
[dev-dependencies]
shared = { path = "../shared", features = ["dev"] }
[target.'cfg(target_os = "linux")'.dependencies]
foreign = { path = "../foreign" }
''',
        "app/src/lib.rs": 'compile_error!("CRATE_MUST_NOT_COMPILE");\n',
        "app/src/bin/runner.rs": 'compile_error!("BINARY_MUST_NOT_COMPILE");\n',
        "app/src/bin/unrelated.rs": 'compile_error!("UNRELATED_BINARY_MUST_NOT_COMPILE");\n',
        "app/src/gated.rs": 'compile_error!("GATED_BINARY_MUST_NOT_COMPILE");\n',
        "app/tests/unrelated.rs": 'compile_error!("UNRELATED_TEST_MUST_NOT_COMPILE");\n',
        "app/build.rs": 'compile_error!("BUILDSCRIPT_MUST_NOT_COMPILE");\nfn main() {}\n',
        "app/tests/integration.rs": 'compile_error!("TEST_MUST_NOT_COMPILE");\n',
        "shared/Cargo.toml": '''[package]
name = "shared"
version = "0.1.0"
edition = "2024"
[features]
target = []
host = []
dev = []
macro = []
requested = []
[lints.rust]
unused = "deny"
''',
        "shared/src/lib.rs": 'compile_error!("DEPENDENCY_MUST_NOT_COMPILE");\n',
        "derive/Cargo.toml": '''[package]
name = "derive"
version = "0.1.0"
edition = "2024"
[lib]
proc-macro = true
[dependencies]
shared = { path = "../shared", features = ["macro"] }
''',
        "derive/src/lib.rs": 'compile_error!("PROC_MACRO_MUST_NOT_COMPILE");\n',
        "foreign/Cargo.toml": '''[package]
name = "foreign"
version = "0.1.0"
edition = "2024"
''',
        "foreign/src/lib.rs": 'compile_error!("INACTIVE_TARGET_MUST_NOT_COMPILE");\n',
    }
    for name, content in files.items():
        destination = FIXTURE / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(content)
    linker = RECEIPTS / "linker-hook.sh"
    RECEIPTS.mkdir(exist_ok=True)
    linker.write_text(f'#!/bin/sh\necho executed > "{RECEIPTS / "linker-executed"}"\nexit 1\n')
    linker.chmod(0o700)
    config = FIXTURE / ".cargo/config.toml"
    config.write_text(config.read_text().replace('[target.aarch64-apple-darwin]', f'[target.aarch64-apple-darwin]\nlinker = {json.dumps(str(linker))}'))


def main() -> None:
    """Compare the helper to Cargo CLI and assert omitted-JSON semantics directly."""
    binary, toolchain = Path(sys.argv[1]).resolve(), Path(sys.argv[2]).resolve()
    write_fixture()
    RECEIPTS.mkdir(exist_ok=True)
    cargo_home = ROOT / "fixture-cargo-home"
    cargo_home.mkdir(exist_ok=True)
    env = {"PATH": f"{toolchain}:/usr/bin:/bin", "CARGO_HOME": str(cargo_home), "RUSTC": str(toolchain / "rustc")}
    subprocess.run([toolchain / "cargo", "generate-lockfile", "--offline"], cwd=FIXTURE, env=env, check=True)
    lock = (FIXTURE / "Cargo.lock").read_bytes()
    results = []
    selections = [
        ({"kind": "package"}, []),
        ({"kind": "library"}, ["--lib"]),
        ({"kind": "binary", "name": "runner"}, ["--bin", "runner"]),
        ({"kind": "integration-test", "name": "integration"}, ["--test", "integration"]),
    ]
    cases = [(mode, profile, selection, flags)
             for mode, profile in [("build", "dev"), ("test", "dev"), ("build", "release"), ("check", "dev")]
             for selection, flags in selections]
    for mode, profile, selection, flags in cases:
        request = {
            "manifest": str(FIXTURE / "Cargo.toml"), "package": "app", "mode": mode, "target_filter": selection, "source_policy": "offline",
            "features": ["shared/requested"], "default_features": True, "all_features": False,
            "target": "aarch64-apple-darwin", "profile": profile,
            "cargo_home": str(cargo_home), "rustc": str(toolchain / "rustc"),
            "target_directory": str(ROOT / "fixture-target"),
        }
        name = f"{mode}-{profile}-{selection['kind']}"
        (RECEIPTS / f"{name}.request.json").write_text(json.dumps(request, indent=2))
        output = subprocess.run([binary], input=json.dumps(request), text=True, capture_output=True, env=env)
        assert output.returncode == 0, (name, output.stderr)
        (RECEIPTS / f"{name}.json").write_text(output.stdout)
        (RECEIPTS / f"{name}.stderr").write_text(output.stderr)
        graph = json.loads(output.stdout)
        command = [str(toolchain / "cargo"), mode, "--unit-graph", "-Z", "unstable-options", "--frozen",
                   "-p", "app", "--target", request["target"], "--profile", profile, "--features", "shared/requested", *flags]
        reference = subprocess.run(command, cwd=FIXTURE, env=env | {"RUSTC_BOOTSTRAP": "1"}, capture_output=True, text=True, check=True)
        (RECEIPTS / f"{name}.cargo.json").write_text(reference.stdout)
        cargo_graph = json.loads(reference.stdout)
        actual = [{"pkg_id": u["package_id"], **{key: u[key] for key in ("target", "platform", "mode", "profile", "features")},
                   "dependencies": [{key: d[key] for key in ("index", "extern_crate_name", "public", "noprelude", "nounused")} for d in u["dependencies"]]} for u in graph["units"]]
        assert actual == cargo_graph["units"], f"{name}: Cargo unit graph differs"
        assert graph["roots"] == cargo_graph["roots"]
        shared = [u for u in graph["units"] if u["target"]["name"] == "shared"]
        host = [u for u in shared if u["platform"] is None]
        target = [u for u in shared if u["platform"] == request["target"]]
        assert len(host) == 1 and target, (name, shared)
        assert host[0]["features"] == ["host", "macro", "requested"]
        needs_dev = mode == "test" or selection["kind"] == "integration-test"
        expected_features = ["dev", "requested", "target"] if needs_dev else ["requested", "target"]
        assert all(unit["features"] == expected_features for unit in target)
        assert host[0]["rustflags"] == []
        assert target[0]["rustflags"] == ["--cfg", "target_marker"]
        assert target[0]["rustdocflags"] == ["--cfg", "doc_marker"]
        assert target[0]["linker"] == str(RECEIPTS / "linker-hook.sh")
        assert not (RECEIPTS / "linker-executed").exists()
        assert "--deny=unused" in target[0]["package_lint_flags"], target[0]["package_lint_flags"]
        assert host[0]["profile"]["opt_level"] == ("2" if profile == "release" else "0")
        assert any(u["target"]["name"] == "build-script-build" and u["platform"] is None and u["profile"]["opt_level"] == "0" for u in graph["units"])
        assert target[0]["profile"]["opt_level"] == ("2" if profile == "release" else "1")
        assert not any(u["target"]["name"] == "foreign" for u in graph["units"])
        assert any(u["target"]["kind"] == ["proc-macro"] and u["platform"] is None for u in graph["units"])
        assert any(u["mode"] == "run-custom-build" for u in graph["units"])
        has_integration = selection["kind"] == "integration-test" or (selection["kind"] == "package" and mode == "test")
        assert any(u["target"]["kind"] == ["test"] for u in graph["units"]) == has_integration
        roots = [graph["units"][index] for index in graph["roots"]]
        if selection["kind"] != "package":
            expected_name = selection.get("name", "app")
            assert len(roots) == 1 and roots[0]["target"]["name"] == expected_name, (name, roots)
        if mode != "test" and selection["kind"] in ["library", "binary"]:
            assert not any(u["target"]["name"] == "unrelated" for u in graph["units"]), name
        assert all(u["target"]["name"] != "gated" for u in graph["units"])
        for unit in (u for u in graph["units"] if u["package_name"] == "app"):
            package_env = unit["package_environment"]
            assert package_env["CARGO_PKG_AUTHORS"] == "First Author:Second Author"
            assert package_env["CARGO_PKG_DESCRIPTION"] == "Planner environment fixture"
            assert package_env["CARGO_PKG_LICENSE"] == "MIT"
            assert package_env["CARGO_PKG_RUST_VERSION"] == "1.90"
        assert (FIXTURE / "Cargo.lock").read_bytes() == lock
        assert not any((ROOT / "fixture-target").rglob("*.rlib"))
        results.append({"case": name, "units": len(graph["units"]), "cargo_parity": True,
                        "host_features": host[0]["features"], "target_features": target[0]["features"]})
    request["target_filter"] = {"kind": "package"}
    output = subprocess.run([binary], input=json.dumps(request), text=True, capture_output=True, env=env, check=True)
    graph = json.loads(output.stdout)
    summary = {"cases": results, "lock_sha256": hashlib.sha256(lock).hexdigest(), "source_compilation": False}
    for selection, flags in [
        ({"kind": "binary", "name": "absent"}, ["--bin", "absent"]),
        ({"kind": "integration-test", "name": "absent"}, ["--test", "absent"]),
        ({"kind": "binary", "name": "gated"}, ["--bin", "gated"]),
    ]:
        rejected = subprocess.run([binary], input=json.dumps(request | {"mode": "build", "target_filter": selection}),
                                  text=True, capture_output=True, env=env)
        reference = subprocess.run([toolchain / "cargo", "build", "--unit-graph", "-Z", "unstable-options", "--frozen", "-p", "app", *flags],
                                   cwd=FIXTURE, env=env | {"RUSTC_BOOTSTRAP": "1"}, text=True, capture_output=True)
        assert rejected.returncode != 0 and reference.returncode != 0, selection
        (RECEIPTS / f"reject-{selection['kind']}-{selection['name']}.stderr").write_text(rejected.stderr)
    summary["missing_or_unenabled_targets_rejected"] = True
    with (FIXTURE / ".cargo/config.toml").open("a") as config:
        config.write('\n[unstable]\nbuild-std = ["std"]\n')
    output = subprocess.run([binary], input=json.dumps(request), text=True, capture_output=True, env=env)
    (RECEIPTS / "stable-ignores-unstable-config.stderr").write_text(output.stderr)
    assert output.returncode == 0, output.stderr
    assert json.loads(output.stdout) == graph
    stable = subprocess.run([toolchain / "cargo", "metadata", "--format-version=1", "--frozen"], cwd=FIXTURE, env=env, capture_output=True, text=True)
    (RECEIPTS / "stable-metadata-ignores-unstable-config.json").write_text(stable.stdout)
    assert stable.returncode == 0 and not any(package["name"] == "std" for package in json.loads(stable.stdout)["packages"]), stable.stderr
    manifest = FIXTURE / "Cargo.toml"
    original_manifest = manifest.read_text()
    manifest.write_text('cargo-features = ["profile-rustflags"]\n' + original_manifest)
    output = subprocess.run([binary], input=json.dumps(request), text=True, capture_output=True, env=env)
    (RECEIPTS / "stable-rejects-nightly-manifest.stderr").write_text(output.stderr)
    assert output.returncode != 0 and "requires a nightly version" in output.stderr, output.stderr
    manifest.write_text(original_manifest)
    assert (FIXTURE / "Cargo.lock").read_bytes() == lock
    summary["stable_config_and_manifest_semantics"] = True
    lock_path = FIXTURE / "Cargo.lock"
    lock_path.unlink()
    output = subprocess.run([binary], input=json.dumps(request), text=True, capture_output=True, env=env)
    lock_path.write_bytes(lock)
    (RECEIPTS / "reject-missing-lock.stderr").write_text(output.stderr)
    assert output.returncode != 0 and "pre-existing Cargo.lock" in output.stderr
    verify_compiler_ownership(binary, request, env)
    summary["compiler_ownership"] = True
    verify_storage_ownership(binary, request, env, graph)
    summary["caller_owned_storage"] = True
    verify_source_home_lease(binary, request, env, graph)
    summary["exclusive_source_home_lease"] = True
    verify_package_environment(binary, toolchain, request, env)
    summary["package_compiler_environment_parity"] = True
    summary["declared_feature_metadata_and_compiler_parity"] = True
    app_manifest = FIXTURE / "app/Cargo.toml"
    original_app = app_manifest.read_text()
    app_manifest.write_text(original_app + '\n[lib]\nharness = false\n')
    output = subprocess.run([binary], input=json.dumps(request | {"mode": "test"}), text=True, capture_output=True, env=env)
    app_manifest.write_text(original_app)
    (RECEIPTS / "reject-custom-harness.stderr").write_text(output.stderr)
    assert output.returncode != 0 and "custom test harness" in output.stderr, output.stderr
    (RECEIPTS / "summary.json").write_text(json.dumps(summary, indent=2) + "\n")
    print(json.dumps(summary, indent=2))


def verify_storage_ownership(binary: Path, request: dict, env: dict, expected: dict) -> None:
    """Cargo's project and ambient storage settings cannot escape caller-owned scratch."""
    config = FIXTURE / ".cargo/config.toml"
    original, lock = config.read_bytes(), (FIXTURE / "Cargo.lock").read_bytes()
    with tempfile.TemporaryDirectory(dir=ROOT) as temporary:
        root = Path(temporary)
        outside = [root / name for name in ["project-build", "project-target", "ambient-build", "ambient-target", "ambient-config-target"]]
        configured = original + f'\n[build]\nbuild-dir={json.dumps(str(outside[0]))}\ntarget-dir={json.dumps(str(outside[1]))}\n'.encode()
        config.write_bytes(configured)
        try:
            for policy in ["offline", "acquire-locked"]:
                for ambient in [False, True]:
                    owned = root / f"owned-{policy}-{ambient}"
                    owned.mkdir()
                    process_env = env | ({"CARGO_BUILD_BUILD_DIR": str(outside[2]), "CARGO_TARGET_DIR": str(outside[3]),
                                          "CARGO_BUILD_TARGET_DIR": str(outside[4])} if ambient else {})
                    selected = request | {"source_policy": policy, "target_directory": str(owned)}
                    output = subprocess.run([binary], input=json.dumps(selected), env=process_env, text=True, capture_output=True)
                    assert output.returncode == 0 and json.loads(output.stdout) == expected, output.stderr
                    assert (owned / ".rustc_info.json").is_file(), list(owned.iterdir())
                    assert all(not path.exists() for path in outside), outside
                    assert config.read_bytes() == configured
                    assert (FIXTURE / "Cargo.lock").read_bytes() == lock
        finally:
            config.write_bytes(original)


def verify_source_home_lease(binary: Path, request: dict, env: dict, expected: dict) -> None:
    """An external lease blocks compiler work, and releasing it preserves the graph."""
    marker, compiler = RECEIPTS / "lease-probe", RECEIPTS / "lease-rustc"
    marker.unlink(missing_ok=True)
    compiler.write_text(f'#!/bin/sh\necho probe > "{marker}"\nexec "{request["rustc"]}" "$@"\n')
    compiler.chmod(0o700)
    selected = request | {"rustc": str(compiler), "target_directory": str(ROOT / "lease-target")}
    with (Path(request["cargo_home"]) / "plan.lock").open("a+") as lease:
        fcntl.flock(lease, fcntl.LOCK_EX)
        with subprocess.Popen([binary], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                              env=env | {"RUSTC": str(compiler)}, text=True) as process:
            try:
                process.communicate(json.dumps(selected), timeout=1)
                raise AssertionError("helper bypassed the external source-home lease")
            except subprocess.TimeoutExpired:
                assert not marker.exists(), "compiler ran while another process held the lease"
            finally:
                fcntl.flock(lease, fcntl.LOCK_UN)
            output, error = process.communicate(timeout=20)
            assert process.returncode == 0 and json.loads(output) == expected, error
        assert marker.exists()
        compiler.write_text(f'#!/bin/sh\nkill -STOP "$PPID"\nexec "{request["rustc"]}" "$@"\n')
        selected["target_directory"] = str(ROOT / "lease-cancel-target")
        with subprocess.Popen([binary], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                              env=env | {"RUSTC": str(compiler)}, text=True) as process:
            try:
                try:
                    process.communicate(json.dumps(selected), timeout=1)
                except subprocess.TimeoutExpired:
                    pass
                try:
                    fcntl.flock(lease, fcntl.LOCK_EX | fcntl.LOCK_NB)
                except BlockingIOError:
                    pass
                else:
                    raise AssertionError("helper did not retain its source-home lease")
            finally:
                process.kill()
                process.communicate()
        fcntl.flock(lease, fcntl.LOCK_EX | fcntl.LOCK_NB)
    lock = Path(request["cargo_home"]) / "plan.lock"
    lock.unlink()
    lock.symlink_to(RECEIPTS / "unowned-lease")
    try:
        rejected = subprocess.run([binary], input=json.dumps(request), env=env, text=True, capture_output=True)
        assert rejected.returncode != 0 and not (RECEIPTS / "unowned-lease").exists(), rejected.stderr
    finally:
        lock.unlink()


def verify_package_environment(binary: Path, toolchain: Path, request: dict, env: dict) -> None:
    """Compare every package variable with env! evaluated by the pinned Cargo compiler."""
    workspace = ROOT / "target/environment-verification"
    (workspace / "src").mkdir(parents=True, exist_ok=True)
    manifest = workspace / "Cargo.toml"
    manifest.write_text('[package]\nname="package-env"\nversion="1.2.3-alpha+meta"\nedition="2024"\n'
                        'authors=["First Author", "Second Author"]\ndescription="Compiler environment"\n'
                        'homepage="https://example.invalid"\nrepository="https://example.invalid/repo"\n'
                        'license="MIT"\nreadme="README.md"\nrust-version="1.90"\n[workspace]\n'
                        '[features]\ndefault=["enabled"]\nenabled=[]\ndisabled=[]\nwith-helper=["dep:helper"]\n'
                        '[dependencies]\nhelper={path="helper",optional=true}\nimplicit={path="implicit",optional=true}\n')
    for name in ["helper", "implicit"]:
        (workspace / name / "src").mkdir(parents=True, exist_ok=True)
        (workspace / name / "Cargo.toml").write_text(f'[package]\nname="{name}"\nversion="0.1.0"\nedition="2024"\n')
        (workspace / name / "src/lib.rs").write_text('compile_error!("DISABLED_DEPENDENCY_MUST_NOT_COMPILE");\n')
    (workspace / "README.md").write_text("Compiler environment fixture.\n")
    keys = ["NAME", "VERSION", "VERSION_MAJOR", "VERSION_MINOR", "VERSION_PATCH", "VERSION_PRE",
            "AUTHORS", "DESCRIPTION", "HOMEPAGE", "REPOSITORY", "LICENSE", "LICENSE_FILE", "README", "RUST_VERSION"]
    source = '#![deny(unexpected_cfgs)]\n#[cfg(feature="disabled")] compile_error!("DISABLED_FEATURE");\nfn main() {\n' + "".join(
        f'println!("CARGO_PKG_{key}={{}}", env!("CARGO_PKG_{key}"));\n' for key in keys) + "}\n"
    (workspace / "src/main.rs").write_text(source)
    subprocess.run([toolchain / "cargo", "generate-lockfile", "--offline"], cwd=workspace, env=env, check=True)
    lock = (workspace / "Cargo.lock").read_bytes()
    request = request | {"manifest": str(manifest), "package": "package-env", "mode": "build",
                         "target_filter": {"kind": "binary", "name": "package-env"}, "features": [],
                         "target": None, "target_directory": str(workspace / "planner-target")}
    planned = subprocess.run([binary], input=json.dumps(request), env=env, text=True, capture_output=True, check=True)
    graph = json.loads(planned.stdout)
    unit = graph["units"][graph["roots"][0]]
    actual = unit["package_environment"]
    metadata = subprocess.run([toolchain / "cargo", "metadata", "--frozen", "--no-deps", "--format-version=1"],
                              cwd=workspace, env=env, text=True, capture_output=True, check=True)
    package = next(package for package in json.loads(metadata.stdout)["packages"] if package["name"] == "package-env")
    assert unit["declared_features"] == sorted(package["features"])
    assert unit["features"] == ["default", "enabled"]
    reference = subprocess.run([toolchain / "cargo", "run", "--frozen", "-vv"],
                               cwd=workspace, env=env, text=True, capture_output=True, check=True)
    assert actual == dict(line.split("=", 1) for line in reference.stdout.splitlines()), actual
    check_cfg = re.findall(r"cfg\(feature, values\(([^)]*)\)\)", reference.stderr)
    assert check_cfg and all(json.loads("[" + value + "]") == unit["declared_features"] for value in check_cfg), reference.stderr
    (RECEIPTS / "declared-features.compiler.stderr").write_text(reference.stderr)
    assert (workspace / "Cargo.lock").read_bytes() == lock
    (RECEIPTS / "package-environment.json").write_text(json.dumps(actual, indent=2) + "\n")


def verify_compiler_ownership(binary: Path, request: dict, env: dict) -> None:
    """A configured compiler hook must be rejected without executing its marker."""
    marker = RECEIPTS / "compiler-hook-executed"
    hook = RECEIPTS / "compiler-hook.sh"
    hook.write_text(f'#!/bin/sh\necho executed >> "{marker}"\nexec "$@"\n')
    hook.chmod(0o700)
    config = FIXTURE / ".cargo/config.toml"
    original = config.read_text()
    cases = [(key, env | {key: str(hook)}, original) for key in
             ["RUSTC", "RUSTC_WRAPPER", "RUSTC_WORKSPACE_WRAPPER", "CARGO_BUILD_RUSTC",
              "CARGO_BUILD_RUSTC_WRAPPER", "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER"]]
    cases.extend((key, env, original + f'\n[build]\n{key} = {json.dumps(str(hook))}\n')
                 for key in ["rustc", "rustc-wrapper", "rustc-workspace-wrapper"])
    cases.append(("env-rustc", env, original + '\n[env]\nRUSTC = { value = "other-rustc", force = true }\n'))
    for name, process_env, content in cases:
        marker.unlink(missing_ok=True)
        config.write_text(content)
        output = subprocess.run([binary], input=json.dumps(request), text=True, capture_output=True, env=process_env)
        (RECEIPTS / f"reject-{name}.stderr").write_text(output.stderr)
        assert output.returncode != 0, f"accepted compiler override {name}"
        assert "compiler" in output.stderr, output.stderr
        assert not marker.exists(), f"executed compiler override {name}"
    config.write_text(original + '\n[env]\nEXPLICIT_BUILD_VALUE="configured"\n')
    output = subprocess.run([binary], input=json.dumps(request), text=True, capture_output=True, env=env)
    assert output.returncode != 0 and "unsupported Cargo environment configuration" in output.stderr
    config.write_text(original)


if __name__ == "__main__":
    main()
