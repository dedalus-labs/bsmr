# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Checks root selection and configured features without compiling source.

import json
import fcntl
from pathlib import Path
import subprocess
import sys
import tempfile


def main() -> None:
    """Resolve build and test graphs against the same immutable lockfile."""
    binary = str(Path(sys.argv[1]).resolve())
    rustc = Path(subprocess.check_output(
        ["rustup", "which", "--toolchain", "1.97.1", "rustc"], text=True
    ).strip())
    with tempfile.TemporaryDirectory(prefix="cargo-plan-") as temporary:
        root = Path(temporary).resolve()
        files = {
            "Cargo.toml": '[workspace]\nmembers=["app","shared","dev"]\nresolver="2"\n',
            "app/Cargo.toml": '[package]\nname="app"\nversion="0.1.0"\nedition="2024"\n[dependencies]\nshared={path="../shared",features=["ordinary"]}\n[dev-dependencies]\ndev={path="../dev"}\n',
            "shared/Cargo.toml": '[package]\nname="shared"\nversion="0.1.0"\nedition="2024"\n[features]\nordinary=[]\n',
            "dev/Cargo.toml": '[package]\nname="dev"\nversion="0.1.0"\nedition="2024"\n',
            "app/build.rs": 'compile_error!("must not compile a build script");\n',
        }
        files.update({f"{name}/src/lib.rs": 'compile_error!("must not compile source");\n' for name in ["app", "shared", "dev"]})
        for path, value in files.items():
            destination = root / path
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_text(value)
        env = {"PATH": f"{rustc.parent}:/usr/bin:/bin", "RUSTC": str(rustc), "CARGO_HOME": str(root / "cargo")}
        subprocess.run([rustc.parent / "cargo", "generate-lockfile", "--offline"], cwd=root, env=env, check=True, capture_output=True)
        lock = (root / "Cargo.lock").read_bytes()
        for mode in ["build", "test"]:
            request = {
                "manifest": str(root / "Cargo.toml"), "package": "app", "mode": mode,
                "target_filter": {"kind": "library"}, "source_policy": "offline",
                "features": [], "default_features": True, "all_features": False,
                "target": None, "profile": "dev", "cargo_home": str(root / "cargo"),
                "rustc": str(rustc), "target_directory": str(root / "target"),
            }
            result = subprocess.run([binary], input=json.dumps(request), env=env, text=True, capture_output=True, check=True, timeout=30)
            graph = json.loads(result.stdout)
            assert graph["schema_version"] == 2 and graph["cargo_library"] == "0.98.0"
            assert "release: 1.97.1" in graph["rustc_version"]
            assert len(graph["roots"]) == 1
            assert graph["units"][graph["roots"][0]]["target"]["name"] == "app"
            assert any(unit["target"]["name"] == "dev" for unit in graph["units"]) == (mode == "test")
            shared = [unit for unit in graph["units"] if unit["target"]["name"] == "shared"]
            assert shared and all(unit["features"] == ["ordinary"] for unit in shared)
            for unit in shared:
                assert unit["declared_features"] == ["ordinary"]
                assert unit["package_environment"]["CARGO_PKG_NAME"] == "shared"
                assert unit["source"]["kind"] == "path"
                assert unit["source"]["manifest"] == str(root / "shared/Cargo.toml")
            assert any(unit["mode"] == "run-custom-build" for unit in graph["units"])
            assert (root / "Cargo.lock").read_bytes() == lock
            assert not list((root / "target").rglob("*.rlib"))
        with (root / "cargo/plan.lock").open("rb") as lease:
            fcntl.flock(lease, fcntl.LOCK_EX)
            with subprocess.Popen([binary], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=env, text=True) as waiting:
                try:
                    waiting.communicate(json.dumps(request), timeout=0.2)
                    raise AssertionError("planner did not wait for source-home ownership")
                except subprocess.TimeoutExpired:
                    fcntl.flock(lease, fcntl.LOCK_UN)
                    stdout, stderr = waiting.communicate(timeout=30)
                    assert waiting.returncode == 0, stderr
                    assert json.loads(stdout)["roots"]
        config = root / ".cargo/config.toml"
        config.parent.mkdir()
        for flags in [["--extern", "untracked=outside.rlib"], ["--cfg", "@outside.args"], ["-Zcodegen-backend=outside.so"], ["-C", "target-feature=+crt-static"]]:
            config.write_text("[build]\nrustflags=" + json.dumps(flags) + "\n")
            result = subprocess.run([binary], input=json.dumps(request), env=env, text=True, capture_output=True, timeout=30)
            assert result.returncode != 0 and "unsupported compiler flag" in result.stderr
            if flags[0] == "-C":
                assert flags[1] in result.stderr and "Compile" in result.stderr
    print("ok: configured roots, features, dev dependencies, and lock preservation without compilation")


if __name__ == "__main__":
    main()
