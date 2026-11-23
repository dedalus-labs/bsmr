# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

"""Require native Go acquisition and synchronization to honor the module's exact pin."""

import json
import os
import re
import shutil
import subprocess
from collections.abc import Iterator
from pathlib import Path

import pytest


class Project:
    """Drive the real binary in one initialized Go module."""

    def __init__(self, root: Path) -> None:
        """Select the candidate binary explicitly instead of using PATH."""
        self.root = root
        self.binary = Path(os.environ["BSMR_BINARY"]).resolve(strict=True)

    def run(self, *args: str) -> subprocess.CompletedProcess[str]:
        """Retain the exit status and diagnostics of a real CLI invocation."""
        result = subprocess.run(
            [self.binary, *args], cwd=self.root, capture_output=True, text=True, check=False
        )
        return result

    def succeed(self, *args: str) -> str:
        """Require success and return the command's output."""
        result = self.run(*args)
        assert result.returncode == 0, result.stdout + result.stderr
        return result.stdout

    def pin(self, directive: str) -> None:
        """Write module metadata without supplying an SDK lock or ambient Go settings."""
        (self.root / "go.mod").write_text(f"module example.com/pin\n\ngo 1.26.0\n{directive}\n")


@pytest.fixture
def project(tmp_path: Path) -> Iterator[Project]:
    """Provide an initialized module and stop its real daemon after each test."""
    project = Project(root=tmp_path)
    project.succeed("init")
    try:
        yield project
    finally:
        project.succeed("kill")


@pytest.mark.parametrize(
    "directive",
    [
        "",
        "toolchain go1.26",
        "toolchain default",
        "toolchain go1.27rc1",
        "toolchain go1.26.1\ntoolchain go1.26.2",
        "toolchain go01.26.1",
    ],
)
@pytest.mark.parametrize("command", ["toolchain", "sync"])
def test_requires_exact_module_pin(project: Project, directive: str, command: str) -> None:
    """Missing or ambiguous pins must fail before reading a lock or acquiring an SDK."""
    project.pin(directive=directive)
    result = project.run("go", command, "--check")
    assert result.returncode != 0
    assert "requires exactly one `toolchain goX.Y.Z`" in result.stderr
    assert not (project.root / ".bsmr-go-toolchain.json").exists()


def test_acquisition_uses_module_pin(project: Project) -> None:
    """Acquire the older pinned SDK, preserve it on replay, and reject pin drift."""
    project.pin(directive="toolchain go1.26.1 // exact SDK")
    assert "Go SDK pinned: 1.26.1" in project.succeed("go", "toolchain")
    assert "Go SDK verified: 1.26.1" in project.succeed("go", "toolchain", "--check")
    lock = (project.root / ".bsmr-go-toolchain.json").read_bytes()
    assert "Go SDK pinned: 1.26.1" in project.succeed("go", "toolchain")
    assert (project.root / ".bsmr-go-toolchain.json").read_bytes() == lock
    conflict = project.run("go", "toolchain", "--version", "1.26.2")
    assert conflict.returncode != 0
    assert "does not match go.mod toolchain go1.26.1" in conflict.stderr
    update = project.run("go", "toolchain", "--update")
    assert update.returncode != 0
    assert "edit the toolchain directive in go.mod" in update.stderr
    project.pin(directive="toolchain go1.26.2")
    for command in [("toolchain", "--check"), ("sync", "--check")]:
        result = project.run("go", *command)
        assert result.returncode != 0
        assert "does not match go.mod toolchain go1.26.2" in result.stderr
    assert (project.root / ".bsmr-go-toolchain.json").read_bytes() == lock
    assert "Go SDK pinned: 1.26.2" in project.succeed("go", "toolchain")
    assert "Go SDK verified: 1.26.2" in project.succeed("go", "toolchain", "--check")


def write_lock(root: Path, version: str) -> bytes:
    """Write a well-formed generated lock whose invented digests no download can match."""
    archives = [
        {
            "os": os_name,
            "arch": arch,
            "filename": f"go{version}.{os_name}-{arch}.tar.gz",
            "sha256": digit * 64,
            "size": 1,
        }
        for (os_name, arch), digit in zip(
            [("darwin", "amd64"), ("darwin", "arm64"), ("linux", "amd64"), ("linux", "arm64")],
            "abcd",
            strict=True,
        )
    ]
    lock = {
        "generated_by": "bsmr go toolchain",
        "schema": 1,
        "version": version,
        "archives": archives,
    }
    content = (json.dumps(lock, indent=2) + "\n").encode()
    (root / ".bsmr-go-toolchain.json").write_bytes(content)
    return content


@pytest.mark.parametrize(
    ("args", "diagnostic"),
    [
        (("--update",), "edit the toolchain directive in go.mod"),
        (("--version", "1.26.2"), "does not match go.mod toolchain go1.26.1"),
    ],
)
def test_rejected_flags_change_nothing(
    project: Project, args: tuple[str, ...], diagnostic: str
) -> None:
    """A rejected request must fail before retiring generated files or touching the lock."""
    project.pin(directive="toolchain go1.26.1")
    (project.root / "toolchains").mkdir()
    lock = write_lock(project.root, version="1.26.1")
    result = project.run("go", "toolchain", *args)
    assert result.returncode != 0
    assert diagnostic in result.stderr
    assert (project.root / "toolchains").is_dir()
    assert (project.root / ".bsmr-go-toolchain.json").read_bytes() == lock


def test_update_without_pin_explains_the_pin(project: Project) -> None:
    """`--update` is rejected as an update, whatever the module says."""
    project.pin(directive="")
    result = project.run("go", "toolchain", "--update")
    assert result.returncode != 0
    assert "edit the toolchain directive in go.mod" in result.stderr


def test_owner_check_precedes_lock_migration(project: Project) -> None:
    """A changed pin must not publish its lock while acquisition cannot replace the tools."""
    project.pin(directive="toolchain go1.26.2")
    lock = write_lock(project.root, version="1.26.1")
    (project.root / ".bsmr-go-tools").mkdir()
    result = project.run("go", "toolchain")
    assert result.returncode != 0
    assert "refusing to overwrite user-owned toolchain file" in result.stderr
    assert (project.root / ".bsmr-go-toolchain.json").read_bytes() == lock


def test_workspace_root_reads_go_work_pin(project: Project) -> None:
    """A root with only go.work synchronizes from the workspace's pin."""
    (project.root / "go.work").write_text("go 1.26.0\ntoolchain go1.26.1\n\nuse ./app\n")
    (project.root / "app").mkdir()
    (project.root / "app/go.mod").write_text("module example.com/app\n\ngo 1.26.0\n")
    (project.root / "app/main.go").write_text("package main\n\nfunc main() {}\n")
    result = project.run("go", "sync", "--check")
    assert result.returncode != 0
    assert "failed to read" in result.stderr
    assert ".bsmr-go-toolchain.json" in result.stderr


def test_published_example_pins_documented_release(tmp_path: Path) -> None:
    """The OCI walkthrough's `--version` must match its example module's pin."""
    repository = Path(__file__).resolve().parents[2]
    walkthrough = (repository / "docs/users/oci.md").read_text()
    match = re.search(r"bsmr go toolchain --version (\S+)", walkthrough)
    assert match is not None
    example = tmp_path / "hello"
    shutil.copytree(repository / "examples/oci/hello", example)
    project = Project(root=example)
    project.succeed("init")
    try:
        result = project.run("go", "toolchain", "--check", "--version", match.group(1))
    finally:
        project.succeed("kill")
    assert result.returncode != 0
    assert "failed to read" in result.stderr
    assert ".bsmr-go-toolchain.json" in result.stderr
