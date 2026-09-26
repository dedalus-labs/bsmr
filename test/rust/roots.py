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
        "Cargo.toml": '[workspace]\nmembers=["a","b","shared"]\ndefault-members=["a","b"]\nresolver="2"\n',
        "rust-toolchain.toml": '[toolchain]\nchannel="1.97.1"\n',
        "shared/Cargo.toml": '[package]\nname="shared"\nversion="0.1.0"\nedition="2024"\n[features]\nfast=[]\n',
        "shared/src/lib.rs": 'pub fn value()->u32 { if cfg!(feature="fast") {9} else {7} }\n',
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
        values = [run(path).stdout.strip() for _, path in sorted(outputs.items())]
        trace = re.search(r"Build ID: ([a-f0-9-]+)", result.stderr)
        assert trace, result.stderr
        actions = run(str(binary), "log", "what-ran", "--trace-id", trace[1], "--format", "json", "--filter-category", "rustc.*", "--no-remote")
        return values, [line for line in actions.stdout.splitlines() if line.strip()]

    try:
        run("cargo", "generate-lockfile", "--offline")
        run("cargo", "build", "--locked", "--offline", "--workspace", "-j", "2")
        assert [run(str(root / "target/debug" / package)).stdout.strip() for package in ("a", "b")] == ["9", "9"]
        run(str(binary), "init")
        values, actions = build("a", "b")
        assert values == ["9", "9"], f"joint selection differs from Cargo: {values}"
        assert len(actions) == 3, f"shared crate compiled more than once: {len(actions)}"
        assert build("b", "a") == (["9", "9"], []), "root order changed compilation"
        assert build("b")[0] == ["7"], "single package reused jointly enabled features"
        assert build("a", "b")[0] == ["9", "9"], "restored selection reused single-package analysis"
        assert build("a", "b")[1] == [], "unchanged joint selection recompiled"
        assert build("a:a", "b:b")[0] == ["9", "9"]
        for package in ("a", "b"):
            source = root / package / "src/main.rs"
            source.write_text(source.read_text() + '#[test] fn joint_feature() { assert_eq!(shared::value(),9); }\n')
        run("cargo", "test", "--locked", "--offline", "--workspace", "-j", "2")
        tests = run(str(binary), "test", "a", "b", "-j", "2", "--console", "simple")
        assert "NO TESTS RAN" not in tests.stderr
        isolated = subprocess.run([binary, "test", "b", "--console", "simple"], cwd=root, env=env, capture_output=True, text=True, timeout=180)
        assert isolated.returncode != 0 and "joint_feature" in isolated.stderr, "single test reused joint feature resolution"
        run(str(binary), "test", "a", "b", "-j", "2", "--console", "simple")
        for package in ("a", "b"):
            manifest = root / package / "Cargo.toml"
            manifest.write_text(manifest.read_text() + '\n[[bin]]\nname="same"\npath="src/main.rs"\n')
        assert build("a:same", "b:same")[0] == ["9", "9"], "equal binary names lost package ownership"
        assert build("b:same")[0] == ["7"]
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
