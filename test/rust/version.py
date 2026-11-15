# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

"""Verify that bsmr built by bsmr reports the same release version as Cargo builds."""

import argparse
import json
import subprocess
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def built_version(binary: Path) -> str:
    """Build //:bsmr with the supplied engine and return the result's `--version` line."""
    build = subprocess.run(
        [binary, "build", "//:bsmr", "--show-full-json-output"],
        cwd=ROOT,
        stdout=subprocess.PIPE,
        text=True,
        check=True,
    )
    executable = next(iter(json.loads(build.stdout).values()))
    version = subprocess.run([executable, "--version"], capture_output=True, text=True, check=True)
    return version.stdout.strip()


def main() -> None:
    """Compare the self-built version with the version Release Please writes for Cargo."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    args = parser.parse_args()
    with (ROOT / "app/bsmr/Cargo.toml").open("rb") as manifest:
        expected = f"bsmr {tomllib.load(manifest)['package']['version']}"
    actual = built_version(args.binary.resolve())
    assert actual == expected, f"self-built bsmr reports {actual!r}, Cargo reports {expected!r}"
    print(f"ok  self-built version: {actual}")


if __name__ == "__main__":
    main()
