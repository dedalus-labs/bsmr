# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Records a configured Cargo unit for native lowering tests.

import json
from pathlib import Path
import subprocess
import sys
import tempfile


def main() -> None:
    """Normalize placement-only fields while retaining Cargo's compiler contract."""
    root = Path(__file__).resolve().parent / "fixtures/unit"
    rustc = Path(subprocess.check_output(["rustup", "which", "--toolchain", "1.97.1", "rustc"], text=True).strip())
    with tempfile.TemporaryDirectory(prefix="cargo-fixture-") as temporary:
        env = {"PATH": f"{rustc.parent}:/usr/bin:/bin", "CARGO_HOME": temporary, "RUSTC": str(rustc)}
        subprocess.run([rustc.parent / "cargo", "generate-lockfile", "--offline"], cwd=root, env=env, check=True, capture_output=True)
        request = {"manifest": str(root / "Cargo.toml"), "packages": ["native_fixture"], "mode": "build", "target_filter": {"kind": "library"}, "source_policy": "offline", "features": [], "default_features": True, "all_features": False, "target": None, "profile": "dev", "cargo_home": temporary, "rustc": str(rustc), "target_directory": str(Path(temporary) / "target")}
        result = subprocess.run([str(Path(sys.argv[1]).resolve())], input=json.dumps(request), text=True, capture_output=True, check=True, env=env)
        graph = json.loads(result.stdout.replace(str(root), "/workspace"))
        graph["rustc_version"] = "release: 1.97.1\n"
        (root.parent / "unit.json").write_text(json.dumps(graph, indent=2) + "\n")


if __name__ == "__main__":
    main()
