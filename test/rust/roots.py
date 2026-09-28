# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

"""Compare joint Cargo selections and warm analysis transitions through real binaries."""

import json
import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile


def qualify(binary: Path, base: Path) -> None:
    """Require joint features, single-root isolation, and one shared compiler action."""
    root = base / "project"
    root.mkdir()
    env = dict(os.environ, BSMR_LOCAL_CACHE_DIR=str(base / "cache"))
    for key in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_TARGET_DIR"):
        env.pop(key, None)
    files = {
        "Cargo.toml": '[workspace]\nmembers=["a","b","shared","unused"]\ndefault-members=["a","b"]\nresolver="2"\n',
        "rust-toolchain.toml": '[toolchain]\nchannel="1.97.1"\n',
        "pyproject.toml": '[project]\nname="packaging"\nversion="0.1.0"\n',
        "shared/Cargo.toml": '[package]\nname="shared"\nversion="0.1.0"\nedition="2024"\n[features]\nfast=[]\n',
        "shared/src/lib.rs": 'pub fn value()->u32 { if cfg!(feature="fast") {9} else {7} }\n',
        "unused/Cargo.toml": '[package]\nname="unused"\nversion="0.1.0"\nedition="2024"\n',
        "unused/src/lib.rs": 'compile_error!("not a default member");\n',
    }
    for package in ("a", "b"):
        features = ',features=["fast"]' if package == "a" else ""
        files[f"{package}/Cargo.toml"] = (
            f'[package]\nname="{package}"\nversion="0.1.0"\nedition="2024"\n'
            f'[dependencies]\nshared={{path="../shared"{features}}}\n'
        )
        files[f"{package}/src/main.rs"] = 'fn main() {println!("{}",shared::value());}\n'
    for name, content in files.items():
        path = root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(content)

    def run(*args: str) -> subprocess.CompletedProcess[str]:
        """Run a bounded command and retain its complete diagnostics on failure."""
        result = subprocess.run(args, cwd=root, env=env, capture_output=True, text=True, timeout=180)
        assert result.returncode == 0, result.stdout + result.stderr
        return result

    def build(*patterns: str) -> tuple[list[str], list[str]]:
        """Read actual binaries and native action records from this exact invocation."""
        result = run(str(binary), "build", *patterns, "--console", "simple", "--show-full-json-output", "-j", "2")
        outputs = json.loads(result.stdout)
        values = [run(path).stdout.strip() for label, path in sorted(outputs.items()) if path and not label.endswith(":lib")]
        trace = re.search(r"Build ID: ([a-f0-9-]+)", result.stderr)
        assert trace, result.stderr
        actions = run(str(binary), "log", "what-ran", "--trace-id", trace[1], "--format", "json", "--filter-category", "rustc.*", "--no-remote")
        return values, [line for line in actions.stdout.splitlines() if line.strip()]

    try:
        run("cargo", "generate-lockfile", "--offline")
        run("cargo", "build", "--locked", "--offline", "-j", "2")
        assert [run(str(root / "target/debug" / package)).stdout.strip() for package in ("a", "b")] == ["9", "9"]
        values, actions = build()
        assert not (root / ".bsmr").exists(), "build initialized the checkout"
        assert all((root / path).read_text() == content for path, content in files.items()), "build changed source inputs"
        assert values == ["9", "9"], f"joint selection differs from Cargo: {values}"
        assert len(actions) == 3, f"shared crate compiled more than once: {len(actions)}"
        assert build(".") == (["9", "9"], []), "directory differs from default selection"
        assert build("b", "a") == (["9", "9"], []), "root order changed compilation"
        assert build("b")[0] == ["7"], "single package reused jointly enabled features"
        assert build("a", "b")[0] == ["9", "9"], "restored selection reused single-package analysis"
        assert build("a", "b")[1] == [], "unchanged joint selection recompiled"
        manifest = root / "Cargo.toml"
        original = manifest.read_text()
        manifest.write_text(original.replace('default-members=["a","b"]', 'default-members=["b"]'))
        assert build()[0] == ["7"], "default member edit reused the old feature set"
        manifest.write_text(original)
        assert build()[0] == ["9", "9"], "restored defaults retained single-package features"
        assert build("a:a", "b:b")[0] == ["9", "9"]
        for package in ("a", "b"):
            source = root / package / "src/main.rs"
            source.write_text(source.read_text() + '#[test] fn joint_feature() { assert_eq!(shared::value(),9); }\n')
        run("cargo", "test", "--locked", "--offline", "-j", "2")
        tests = run(str(binary), "test", "a", "b", "-j", "2", "--console", "simple")
        assert "NO TESTS RAN" not in tests.stderr
        tests = run(str(binary), "test", "-j", "2", "--console", "simple")
        assert "NO TESTS RAN" not in tests.stderr
        isolated = subprocess.run([binary, "test", "b", "--console", "simple"], cwd=root, env=env, capture_output=True, text=True, timeout=180)
        assert isolated.returncode != 0 and "joint_feature" in isolated.stderr, "single test reused joint feature resolution"
        run(str(binary), "test", "a", "b", "-j", "2", "--console", "simple")
        for package in ("a", "b"):
            manifest = root / package / "Cargo.toml"
            manifest.write_text(manifest.read_text() + '\n[[bin]]\nname="same"\npath="src/main.rs"\n')
        assert build("a:same", "b:same")[0] == ["9", "9"], "equal binary names lost package ownership"
        assert build("b:same")[0] == ["7"]
        manifest = root / "b/Cargo.toml"
        manifest.write_text(manifest.read_text() + '\n[[bin]]\nname="helper"\npath="src/main.rs"\n')
        assert build("b")[0] == ["7", "7"], "directory lost one of its binaries"
        nested = subprocess.run([binary, "build", "--show-full-json-output", "--console", "simple"], cwd=root / "b", env=env, capture_output=True, text=True, timeout=180)
        assert nested.returncode == 0, nested.stderr
        assert sorted(json.loads(nested.stdout)) == ["root//b:helper", "root//b:same"]
        manifest.write_text(manifest.read_text() + '\n[features]\nextra=[]\n[[bin]]\nname="optional"\npath="src/main.rs"\nrequired-features=["extra"]\n')
        run("cargo", "build", "-p", "b", "--locked", "--offline")
        assert build("b")[0] == ["7", "7"], "disabled optional binary was selected"
        assert build("b", "-c", "rust.features=b/extra")[0] == ["7", "7", "7"]
        for patterns in [("b:optional",), ("b", "b:optional")]:
            required = subprocess.run([binary, "build", *patterns], cwd=root, env=env, capture_output=True, text=True, timeout=180)
            assert required.returncode != 0, "explicit disabled target was silently omitted"
        (root / "b/src/lib.rs").write_text('pub fn enabled() {}\n')
        app = root / "a/Cargo.toml"
        app.write_text(app.read_text() + '\n[dependencies.b]\npath="../b"\nfeatures=["extra"]\n')
        run("cargo", "generate-lockfile", "--offline")
        run("cargo", "build", "--locked", "--offline", "-p", "a", "-p", "b")
        assert build("a", "b")[0] == ["9", "9", "9", "9"], "joint dependency features failed to enable optional target"
        assert build("b")[0] == ["7", "7"], "single selection retained optional target"
        manifest = root / "Cargo.toml"
        manifest.write_text(manifest.read_text().replace('"unused"]', '"unused","only"]'))
        (root / "only/src").mkdir(parents=True)
        (root / "only/Cargo.toml").write_text('[package]\nname="only"\nversion="0.1.0"\nedition="2024"\n[features]\nextra=[]\n[[bin]]\nname="only"\npath="src/main.rs"\nrequired-features=["extra"]\n[[test]]\nname="gated"\npath="gated.rs"\nrequired-features=["extra"]\n')
        (root / "only/src/main.rs").write_text('fn main(){println!("13");}\n#[test] fn enabled(){}\n')
        (root / "only/gated.rs").write_text('#[test] fn enabled(){}\n')
        run("cargo", "generate-lockfile", "--offline")
        assert build("only") == ([], []), "disabled directory created an artifact"
        assert build("only", "-c", "rust.features=only/extra")[0] == ["13"]
        skipped = run(str(binary), "test", "only", "--console", "simple")
        assert "NO TESTS RAN" in skipped.stderr
        enabled = run(str(binary), "test", "only", "-c", "rust.features=only/extra", "--console", "simple")
        assert "Pass 2" in enabled.stderr, enabled.stderr
        run(str(binary), "init")
        config = root / ".bsmr"
        config.write_text(config.read_text() + '\n[alias]\nb = root//a:same\n')
        assert build("b")[0] == ["9"], "directory shadowed an explicit alias"
        (root / "BUILD.bsmr").write_text('filegroup(name="manual", srcs=[])\n')
        explicit = subprocess.run([binary, "build"], cwd=root, env=env, capture_output=True, text=True, timeout=180)
        assert explicit.returncode != 0, "Cargo defaults overrode an explicit build file"
        (root / "BUILD.bsmr").unlink()
        config.unlink()
        manifest = root / "Cargo.toml"
        manifest.write_text(manifest.read_text().replace('default-members=["a","b"]\n', '') + '\n[package]\nname="central"\nversion="0.1.0"\nedition="2024"\n')
        (root / "src").mkdir()
        (root / "src/main.rs").write_text('fn main(){println!("17");}\n')
        run("cargo", "generate-lockfile", "--offline")
        run("cargo", "build", "--locked", "--offline")
        assert build()[0] == ["17"], "non-virtual workspace did not select its root package"
        assert not config.exists(), "build recreated removed configuration"
        print("ok: joint features, shared compiler work, warm selection changes, named roots, duplicate binary names")
    finally:
        run(str(binary), "kill")


def main() -> None:
    """Use one tested engine/planner pair without modifying its source checkout."""
    binary = Path(sys.argv[1]).resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix="bsmr-roots-") as temporary:
        qualify(binary, Path(temporary).resolve())


if __name__ == "__main__":
    main()
