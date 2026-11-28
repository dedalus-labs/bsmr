# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

"""Keep native Cargo acquisition below the Bessemer user root across clean builds."""

import json
import os
import shutil
import subprocess
from collections.abc import Iterator
from pathlib import Path

import pytest


class Project:
    """Run native Rust builds with an isolated home and the installed compiler."""

    def __init__(self, root: Path) -> None:
        """Copy the locked registry fixture without sharing mutable Cargo state."""
        self.binary = Path(os.environ["BSMR_BINARY"]).resolve(strict=True)
        self.root = root / "project"
        self.home = root / "home"
        self.platform_cache = root / "platform-cache"
        self.home.mkdir()
        shutil.copytree(Path(__file__).parents[1] / "fixtures" / "cargo-store", self.root)
        self.env = os.environ | {
            "HOME": str(self.home),
            "XDG_CACHE_HOME": str(self.platform_cache),
            "RUSTUP_HOME": os.environ.get("RUSTUP_HOME", str(Path.home() / ".rustup")),
        }
        self.env.pop("BSMR_LOCAL_CACHE_DIR", None)
        self.env.pop("BSMR_HTTP_CACHE_DIR", None)

    def run(self, *args: str) -> subprocess.CompletedProcess[str]:
        """Require a successful real CLI invocation and retain failure diagnostics."""
        result = subprocess.run(
            [self.binary, "--isolation-dir", "cargo-store", *args],
            cwd=self.root,
            env=self.env,
            capture_output=True,
            text=True,
        )
        assert result.returncode == 0, result.stdout + result.stderr
        return result


@pytest.fixture
def project(tmp_path: Path) -> Iterator[Project]:
    """Stop only this fixture's daemon when the native build test finishes."""
    project = Project(root=tmp_path)
    try:
        yield project
    finally:
        project.run("kill")


def test_cargo_store(project: Project) -> None:
    """A locked registry crate survives clean and a fresh plan without being rewritten."""
    result = project.run("build", "--show-full-json-output")
    outputs: dict[str, str] = json.loads(result.stdout)
    output = Path(outputs["root//:cargo-store"])
    executable = subprocess.run([output], capture_output=True, text=True, check=True)
    assert executable.stdout == "42\n"
    cache = project.home / ".bsmr" / "cargo" / "0.98.0"
    archives = list(cache.glob("registry/cache/*/itoa-1.0.15.crate"))
    assert len(archives) == 1
    archive = archives[0]
    before = archive.stat()
    assert not (project.platform_cache / "bsmr" / "cargo").exists()
    assert not (project.root / ".bsmr-planner").exists()
    assert list((project.home / ".bsmr" / "cargo-planner").iterdir())
    project.run("clean")
    project.run("build", "--show-full-json-output")
    after = archive.stat()
    assert (after.st_ino, after.st_mtime_ns, after.st_size) == (
        before.st_ino,
        before.st_mtime_ns,
        before.st_size,
    )
