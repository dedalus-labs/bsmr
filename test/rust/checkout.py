# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

"""Require a captured Git repository to remain readable after its refs are packed."""

import argparse
import hashlib
import json
import shutil
import tempfile
from pathlib import Path

from macros import run


def git(root: Path, *arguments: str) -> str:
    """Run fixture Git with an explicit identity and retain the original failure."""
    result = run(
        root,
        "git",
        "-c",
        "user.name=Fixture",
        "-c",
        "user.email=fixture@example.invalid",
        "-c",
        "commit.gpgsign=false",
        *arguments,
    )
    assert result.returncode == 0, result.stderr
    return result.stdout.strip()


def qualify(binary: str, root: Path, prelude: Path | None, *, packed: bool) -> None:
    """Materialize the real checkout rule, then ask Git to read its packed HEAD."""
    source = root / "source"
    source.mkdir()
    git(source, "init", "-b", "main")
    git(source, "commit", "--allow-empty", "-m", "fixture")
    if packed:
        git(source, "pack-refs", "--all", "--prune")
    expected = git(source, "rev-parse", "HEAD")
    assert any(path.is_file() for path in (source / ".git/refs").rglob("*")) != packed
    files = {".git/HEAD": source / ".git/HEAD"}
    if packed:
        files[".git/packed-refs"] = source / ".git/packed-refs"
    for directory in ["objects", "refs"]:
        for path in (source / ".git" / directory).rglob("*"):
            if path.is_file():
                files[".git/" + path.relative_to(source / ".git").as_posix()] = path
    inputs = {
        name: [path.as_uri(), hashlib.sha256(path.read_bytes()).hexdigest(), path.stat().st_size]
        for name, path in files.items()
    }
    initialized = run(root, binary, "init")
    assert initialized.returncode == 0, initialized.stderr
    if prelude is not None:
        shutil.copytree(prelude, root / "prelude")
        (root / ".bsmr.local").write_text("[external_cells]\nprelude=disabled\n")
    (root / "BUILD.bsmr").write_text(
        'load("@prelude//rust:checkout.bzl", "cargo_checkout")\n'
        f'cargo_checkout(name="tree", packages={{}}, git={json.dumps(inputs)}, unavailable=[])\n'
    )
    try:
        result = run(root, binary, "build", "//:tree", "--show-full-json-output")
        assert result.returncode == 0, result.stderr
        tree = root / next(iter(json.loads(result.stdout).values()))
        assert git(tree, "rev-parse", "HEAD") == expected
        print(f"ok: captured references remain readable (packed={packed})")
    finally:
        stopped = run(root, binary, "kill")
        assert stopped.returncode == 0, stopped.stderr


def main() -> None:
    """Select bundled rules or an explicit local prelude for development."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("--prelude", type=Path)
    arguments = parser.parse_args()
    for packed in [False, True]:
        with tempfile.TemporaryDirectory(prefix="bsmr-checkout-") as directory:
            qualify(
                str(arguments.binary.resolve()), Path(directory), arguments.prelude, packed=packed
            )


if __name__ == "__main__":
    main()
