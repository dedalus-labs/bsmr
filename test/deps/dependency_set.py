# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

"""Qualify DependencySet lockfiles, updates, build gating, and release receipts."""

import hashlib
import json
import os
import subprocess
import sys
import tempfile
from dataclasses import dataclass
from pathlib import Path

SHA_A = "1111111111111111111111111111111111111111"
SHA_B = "2222222222222222222222222222222222222222"
SHA_C = "3333333333333333333333333333333333333333"
DIGEST_1 = "sha256:" + "1" * 64
DIGEST_2 = "sha256:" + "2" * 64
DIGEST_3 = "sha256:" + "3" * 64
DIGEST_4 = "sha256:" + "4" * 64


@dataclass(frozen=True)
class CliRunner:
    """Runs the real bsmr binary inside an isolated temporary project."""

    binary: str
    root: Path
    env: dict[str, str]

    def run(self, *args: str, expect_ok: bool = True) -> subprocess.CompletedProcess[str]:
        """Execute bsmr and assert exit status."""
        res = subprocess.run(
            [self.binary, *args],
            cwd=self.root,
            env=self.env,
            text=True,
            capture_output=True,
            timeout=120,
        )
        if expect_ok and res.returncode != 0:
            raise AssertionError(f"bsmr {' '.join(args)} failed:\n{res.stderr}\n{res.stdout}")
        if not expect_ok and res.returncode == 0:
            raise AssertionError(f"bsmr {' '.join(args)} unexpectedly succeeded:\n{res.stdout}")
        return res


def manifest_text(*, alt_formatting: bool = False) -> str:
    """Return a valid component manifest in two formatting variants."""
    if alt_formatting:
        return (
            "# equivalent declaration with comments and reordered tables\n"
            'name = "app"\n'
            "schema = 1\n"
            "\n"
            "[requires]\n"
            '"example.kernel.uapi" = { versions = [7, 7] }\n'
            "\n"
            "[provides]\n"
            '"example.app.api" = 1\n'
        )
    return (
        "schema = 1\n"
        'name = "app"\n'
        "\n"
        "[provides]\n"
        '"example.app.api" = 1\n'
        "\n"
        "[requires]\n"
        '"example.kernel.uapi" = { min = 7, max = 7 }\n'
    )


def lock_text(
    *,
    kernel_version: int = 7,
    kernel_rev: str = SHA_A,
    kernel_tree: str = DIGEST_1,
    overlay: str | None = None,
    reordered: bool = False,
) -> str:
    """Return a DependencyLock TOML fixture for the app, kernel, and web components."""
    overlay_line = f'overlay = "{overlay}"\n' if overlay is not None else ""
    kernel = (
        "[[components]]\n"
        'name = "kernel"\n'
        'repository = "https://example.com/kernel"\n'
        f'revision = "{kernel_rev}"\n'
        f'tree = "{kernel_tree}"\n'
        f'manifest = "{DIGEST_2}"\n'
        "patches = []\n"
        f"{overlay_line}"
        "[components.provides]\n"
        f'"example.kernel.uapi" = {kernel_version}\n'
        "[[components.artifacts]]\n"
        'name = "vmlinux"\n'
        f'digest = "{DIGEST_3}"\n'
    )
    web = (
        "[[components]]\n"
        'name = "web"\n'
        'repository = "https://example.com/web"\n'
        f'revision = "{SHA_C}"\n'
        f'tree = "{DIGEST_2}"\n'
        f'manifest = "{DIGEST_2}"\n'
        "patches = []\n"
        "[components.provides]\n"
        '"example.web.ui" = 1\n'
    )
    blocks = [web, kernel] if reordered else [kernel, web]
    return "schema = 1\n\n" + "\n".join(blocks)


def write_catalog(root: Path) -> tuple[Path, str]:
    """Write a deterministic candidate catalog and return its path and SHA-256 digest."""
    body = (
        "schema = 1\n\n"
        "[[candidates]]\n"
        'component = "kernel"\n'
        "release = 12\n"
        'repository = "https://example.com/kernel"\n'
        f'revision = "{SHA_C}"\n'
        f'tree = "{DIGEST_4}"\n'
        f'manifest = "{DIGEST_2}"\n'
        "[candidates.provides]\n"
        '"example.kernel.uapi" = 8\n\n'
        "[[candidates]]\n"
        'component = "kernel"\n'
        "release = 11\n"
        'repository = "https://example.com/kernel"\n'
        f'revision = "{SHA_B}"\n'
        f'tree = "{DIGEST_3}"\n'
        f'manifest = "{DIGEST_2}"\n'
        "[candidates.provides]\n"
        '"example.kernel.uapi" = 7\n'
    )
    path = root / "catalog.toml"
    path.write_text(body)
    digest = "sha256:" + hashlib.sha256(path.read_bytes()).hexdigest()
    return path, digest


def verify_equivalent_identities(cli: CliRunner) -> dict[str, object]:
    """Verify formatting-independent DependencySet and DependencyLock digests."""
    (cli.root / "bsmr.component.toml").write_text(manifest_text())
    (cli.root / "bsmr.lock").write_text(lock_text())
    first = json.loads(cli.run("deps", "verify", "--json").stdout)

    (cli.root / "bsmr.component.toml").write_text(manifest_text(alt_formatting=True))
    (cli.root / "bsmr.lock").write_text(lock_text(reordered=True))
    second = json.loads(cli.run("deps", "verify", "--json").stdout)

    assert first["dependency_set_digest"] == second["dependency_set_digest"]
    assert first["dependency_lock_digest"] == second["dependency_lock_digest"]
    return first


def verify_build_gate_and_explain(cli: CliRunner) -> None:
    """Prove incompatible locks fail bsmr build before analysis and explain reports why."""
    (cli.root / "ok.bzl").write_text(
        "def _impl(ctx):\n"
        '    out = ctx.actions.write("ok.txt", "ok\\n")\n'
        "    return [DefaultInfo(default_output = out)]\n"
        "ok_rule = rule(impl = _impl, attrs = {})\n"
    )
    (cli.root / "BUILD.bsmr").write_text('load(":ok.bzl", "ok_rule")\nok_rule(name = "ok")\n')
    cli.run("init")
    cli.run("build", "//:ok")

    (cli.root / "bsmr.lock").write_text(lock_text(kernel_version=8))
    cli.run("deps", "verify", expect_ok=False)
    failed = cli.run("build", "//:ok", expect_ok=False)
    assert "example.kernel.uapi" in failed.stderr

    explained = json.loads(cli.run("deps", "explain", "--json").stdout)
    assert explained["valid"] is False
    assert any(
        c["contract"] == "example.kernel.uapi" and not c["satisfied"]
        for c in explained["contracts"]
    )


def verify_update_and_receipt(cli: CliRunner, baseline_web_closure: str) -> None:
    """Prove catalog update selects newest compatible release and receipts reject overlays."""
    (cli.root / "bsmr.component.toml").write_text(manifest_text())
    (cli.root / "bsmr.lock").write_text(lock_text())
    catalog_path, catalog_digest = write_catalog(cli.root)

    # Wrong catalog digest must fail closed.
    cli.run(
        "deps",
        "update",
        "kernel",
        "--catalog",
        str(catalog_path),
        "--catalog-digest",
        DIGEST_1,
        expect_ok=False,
    )

    updated = json.loads(
        cli.run(
            "deps",
            "update",
            "kernel",
            "--catalog",
            str(catalog_path),
            "--catalog-digest",
            catalog_digest,
            "--json",
        ).stdout
    )
    report = updated["reports"][0]
    assert report["selected_release"] == 11
    assert report["selected_revision"] == SHA_B
    assert report["decisions"][0]["deciding_contract"] == "example.kernel.uapi"
    # Updating kernel must not change the unrelated web component's closure digest.
    assert updated["verification"]["component_closures"]["web"] == baseline_web_closure

    artifacts_path = cli.root / "artifacts.json"
    artifacts_path.write_text(
        json.dumps(
            [
                {
                    "target": "api",
                    "digest": DIGEST_3,
                    "image": f"registry.example.com/app@{DIGEST_3}",
                }
            ]
        )
    )

    # Local overlay builds succeed, but receipt creation rejects uncommitted overlays.
    (cli.root / "bsmr.lock").write_text(lock_text(kernel_rev=SHA_B, overlay="../kernel-local"))
    cli.run("build", "//:ok")
    cli.run(
        "deps",
        "receipt",
        "create",
        "--artifacts",
        str(artifacts_path),
        "--source-commit",
        SHA_A,
        "--builder",
        "ci-runner",
        "--workflow",
        "build",
        expect_ok=False,
    )

    # Clean lock certifies once and refuses overwrite on second write.
    (cli.root / "bsmr.lock").write_text(lock_text(kernel_rev=SHA_B, kernel_tree=DIGEST_3))
    receipt_path = cli.root / "release-receipt.json"
    cli.run(
        "deps",
        "receipt",
        "create",
        "--artifacts",
        str(artifacts_path),
        "--source-commit",
        SHA_A,
        "--builder",
        "ci-runner",
        "--workflow",
        "build",
        "--output",
        str(receipt_path),
    )
    cli.run(
        "deps",
        "receipt",
        "create",
        "--artifacts",
        str(artifacts_path),
        "--source-commit",
        SHA_A,
        "--builder",
        "ci-runner",
        "--workflow",
        "build",
        "--output",
        str(receipt_path),
        expect_ok=False,
    )
    verified = json.loads(cli.run("deps", "receipt", "verify", str(receipt_path), "--json").stdout)
    assert verified["source_commit"] == SHA_A
    assert len(verified["artifacts"]) == 1


def verify_native_lockfile_binding(cli: CliRunner) -> None:
    """Prove bound native lockfiles fail closed when stale and refresh cleanly."""
    cargo_lock = cli.root / "Cargo.lock"
    pnpm_lock = cli.root / "pnpm-lock.yaml"
    cargo_lock.write_text("version = 4\n")
    pnpm_lock.write_text("lockfileVersion: '9.0'\n")
    cargo_sha = "sha256:" + hashlib.sha256(cargo_lock.read_bytes()).hexdigest()
    pnpm_sha = "sha256:" + hashlib.sha256(pnpm_lock.read_bytes()).hexdigest()

    (cli.root / "bsmr.component.toml").write_text(manifest_text())
    (cli.root / "bsmr.lock").write_text(
        lock_text(kernel_rev=SHA_B, kernel_tree=DIGEST_3) + "\n[[components.lockfiles]]\n"
        'ecosystem = "pnpm"\n'
        'path = "pnpm-lock.yaml"\n'
        f'digest = "{pnpm_sha}"\n\n'
        "[[components]]\n"
        'name = "rust-svc"\n'
        'repository = "https://example.com/rust"\n'
        f'revision = "{SHA_A}"\n'
        f'tree = "{DIGEST_1}"\n'
        f'manifest = "{DIGEST_2}"\n'
        "patches = []\n"
        "[[components.lockfiles]]\n"
        'ecosystem = "cargo"\n'
        'path = "Cargo.lock"\n'
        f'digest = "{cargo_sha}"\n'
    )
    before = json.loads(cli.run("deps", "verify", "--json").stdout)
    cargo_lock.write_text("version = 4\n# bumped\n")
    cli.run("deps", "verify", expect_ok=False)
    cli.run("build", "//:ok", expect_ok=False)

    refreshed = json.loads(cli.run("deps", "update", "--refresh-lockfiles", "--json").stdout)
    assert refreshed["refreshed_lockfiles"] == 1
    after = refreshed["verification"]
    assert after["component_closures"]["rust-svc"] != before["component_closures"]["rust-svc"]
    assert after["component_closures"]["web"] == before["component_closures"]["web"]
    cli.run("build", "//:ok")


def main() -> None:
    """Run all DependencySet end-to-end qualification checks."""
    binary = str(Path(sys.argv[1]).resolve())
    with tempfile.TemporaryDirectory(prefix="bsmr-depset-") as temporary:
        root = Path(temporary)
        env = dict(os.environ, BSMR_LOCAL_CACHE_DIR=str(root / "cache"))
        cli = CliRunner(binary=binary, root=root, env=env)
        try:
            first = verify_equivalent_identities(cli)
            closures = first["component_closures"]
            assert isinstance(closures, dict)
            web_closure = closures["web"]
            assert isinstance(web_closure, str)
            verify_build_gate_and_explain(cli)
            verify_update_and_receipt(cli, web_closure)
            verify_native_lockfile_binding(cli)
            print(
                "PASS: canonical identity, build gate, explain, update, "
                "closure isolation, receipt, native lockfiles"
            )
        finally:
            subprocess.run([binary, "kill"], cwd=root, env=env, capture_output=True, timeout=60)


if __name__ == "__main__":
    main()
