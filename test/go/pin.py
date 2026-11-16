# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

"""Require native Go acquisition and synchronization to honor the module's exact pin."""

import json
import os
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


def test_sdk_is_an_action_input(project: Project) -> None:
    """Build and restore without a source-tree SDK or wrapper, then observe a changed SDK pin."""
    shutil.copyfile(
        Path(__file__).parents[1] / "fixtures/go-action/main.go", project.root / "main.go"
    )
    for version in ["1.26.1", "1.26.2"]:
        project.pin(directive=f"toolchain go{version}")
        project.succeed("go", "toolchain", "--version", version)
        project.succeed("go", "sync")
        shutil.rmtree(project.root / ".bsmr-go-sdk")
        previous = None
        for _ in range(2):
            outputs = json.loads(
                project.succeed(
                    "build", "//:bin", "-c", "go.link_mode=internal", "--show-full-json-output"
                )
            )
            binary = Path(outputs["root//:bin"])
            result = subprocess.run([binary], capture_output=True, text=True, check=True)
            assert result.stdout.strip().split("-X:", 1)[0] == f"go{version}"
            content = binary.read_bytes()
            if previous is not None:
                assert content == previous
            previous = content
            assert not (project.root / ".bsmr-go-sdk").exists()
            assert not (project.root / ".bsmr-go-tools").exists()
            project.succeed("clean")
