# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Compares fresh-daemon Rust graph discovery with identical target results.

import argparse
import hashlib
import json
import os
import platform
import statistics
import subprocess
import tempfile
import time
from pathlib import Path


def digest(path: Path) -> str:
    """Hash the tested executable without loading it into memory."""
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def populate(project: Path, channel: str) -> None:
    """Create the fixed 128-package workload without generated build files."""
    members = [f"p{i:03}" for i in range(128)]
    (project / "Cargo.toml").write_text(
        "[workspace]\nmembers=" + json.dumps(members) + '\nresolver="2"\n'
    )
    (project / "rust-toolchain.toml").write_text(
        f"[toolchain]\nchannel={json.dumps(channel)}\n"
    )
    for name in members:
        directory = project / name
        (directory / "src").mkdir(parents=True)
        (directory / "Cargo.toml").write_text(
            f'[package]\nname="{name}"\nversion="0.1.0"\nedition="2024"\n'
        )
        (directory / "src/lib.rs").write_text("pub fn value() -> u32 { 7 }\n")
        for i in range(128):
            (directory / f"src/module{i:03}.rs").write_text(
                f"pub fn value() -> u32 {{ {i} }}\n"
            )


def measure(binary: Path, project: Path, env: dict[str, str], log: Path) -> float:
    """Time discovery, retain its output and require the expected target."""
    start = time.monotonic()
    result = subprocess.run(
        [binary, "targets", "p127"],
        cwd=project,
        env=env,
        capture_output=True,
        text=True,
        timeout=180,
        check=False,
    )
    elapsed = time.monotonic() - start
    log.write_text(result.stdout + result.stderr)
    result.check_returncode()
    if result.stdout.strip() != "root//p127:p127":
        raise ValueError(f"unexpected target: {result.stdout}")
    return elapsed


def main() -> None:
    """Retain fixtures, binary identities, logs and alternating paired samples."""
    parser = argparse.ArgumentParser(
        description="Compare fresh-daemon Rust graph discovery."
    )
    parser.add_argument("--before", type=Path, required=True)
    parser.add_argument("--after", type=Path, required=True)
    parser.add_argument("--cargo", type=Path, required=True)
    parser.add_argument("--channel", default="1.97.1")
    parser.add_argument("--runs", type=int, default=6)
    args = parser.parse_args()
    if args.runs < 3:
        parser.error("at least three samples per binary are required")
    binaries = {"before": args.before.resolve(), "after": args.after.resolve()}
    cargo = args.cargo.resolve()
    root = Path(tempfile.mkdtemp(prefix="bsmr-rust-graph-")).resolve()
    project = root / "project"
    project.mkdir()
    env = dict(os.environ, PATH=f"{cargo.parent}{os.pathsep}{os.environ['PATH']}")
    env["BSMR_LOCAL_CACHE_DIR"] = str(root / "cache")
    populate(project, args.channel)
    subprocess.run(
        [cargo, "generate-lockfile", "--offline"], cwd=project, env=env, check=True
    )
    subprocess.run([binaries["before"], "init"], cwd=project, env=env, check=True)
    samples: dict[str, list[float]] = {label: [] for label in binaries}
    print(root, flush=True)
    try:
        for sample in range(args.runs):
            order = ["before", "after"] if sample % 2 == 0 else ["after", "before"]
            for label in order:
                binary = binaries[label]
                subprocess.run(
                    [binary, "kill"],
                    cwd=project,
                    env=env,
                    capture_output=True,
                    check=True,
                )
                elapsed = measure(binary, project, env, root / f"{label}-{sample}.log")
                samples[label].append(elapsed)
                print(label, sample, elapsed, flush=True)
        report = {
            "platform": platform.platform(),
            "packages": 128,
            "rustFiles": 16512,
            "mode": "fresh daemon, warm filesystem cache, target discovery only",
            "sha256": {label: digest(path) for label, path in binaries.items()},
            "samples": samples,
            "median": {
                label: statistics.median(values) for label, values in samples.items()
            },
        }
        (root / "results.json").write_text(json.dumps(report, indent=2) + "\n")
    finally:
        for binary in binaries.values():
            subprocess.run(
                [binary, "kill"], cwd=project, env=env, capture_output=True, check=True
            )


if __name__ == "__main__":
    main()
