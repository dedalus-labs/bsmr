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

    def __init__(self, root: Path, home: Path, cache: Path) -> None:
        """Select the candidate binary and give it a private user root over shared caches."""
        self.root = root
        self.home = home
        self.cache = cache
        self.binary = Path(os.environ["BSMR_BINARY"]).resolve(strict=True)

    def run(self, *args: str) -> subprocess.CompletedProcess[str]:
        """Retain the exit status and diagnostics of a real CLI invocation."""
        result = subprocess.run(
            [self.binary, *args],
            cwd=self.root,
            env={**os.environ, "HOME": str(self.home), "XDG_CACHE_HOME": str(self.cache)},
            capture_output=True,
            text=True,
            check=False,
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


@pytest.fixture(scope="module")
def cache(tmp_path_factory: pytest.TempPathFactory) -> Path:
    """Share downloads and action outputs across this module's projects."""
    return tmp_path_factory.mktemp("cache")


@pytest.fixture
def project(tmp_path: Path, cache: Path) -> Iterator[Project]:
    """Provide an initialized module and stop its real daemon after each test."""
    (tmp_path / "home").mkdir()
    (tmp_path / "module").mkdir()
    project = Project(root=tmp_path / "module", home=tmp_path / "home", cache=cache)
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
    """Missing or ambiguous pins must fail before reading metadata or acquiring an SDK."""
    project.pin(directive=directive)
    result = project.run("go", command, "--check")
    assert result.returncode != 0
    assert "requires exactly one `toolchain goX.Y.Z`" in result.stderr
    assert not (project.home / ".bsmr/bootstrap").exists()


def test_acquisition_uses_module_pin(project: Project) -> None:
    """Acquire the older pinned SDK, preserve it on replay, and reject pin drift."""
    project.pin(directive="toolchain go1.26.1 // exact SDK")
    assert "Go SDK pinned: 1.26.1" in project.succeed("go", "toolchain")
    assert "Go SDK verified: 1.26.1" in project.succeed("go", "toolchain", "--check")
    record = project.home / ".bsmr/bootstrap/go/go1.26.1.json"
    recorded = record.read_bytes()
    assert "Go SDK pinned: 1.26.1" in project.succeed("go", "toolchain")
    assert record.read_bytes() == recorded
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
        assert 'go1.26.2.json" is missing or invalid' in result.stderr
    assert record.read_bytes() == recorded
    assert "Go SDK pinned: 1.26.2" in project.succeed("go", "toolchain")
    assert "Go SDK verified: 1.26.2" in project.succeed("go", "toolchain", "--check")


def test_acquisition_writes_no_sdk_copy(project: Project) -> None:
    """Acquire and sync from the archive output, retiring only a marked earlier SDK copy."""
    project.pin(directive="toolchain go1.26.1")
    shutil.copyfile(
        Path(__file__).parents[1] / "fixtures/go-action/main.go", project.root / "main.go"
    )
    user = project.root / ".bsmr-go-sdk"
    user.mkdir()
    (user / "notes.txt").write_bytes(b"user-owned\n")
    project.succeed("go", "toolchain")
    assert "Go graph: 1 packages" in project.succeed("go", "sync")
    assert [path.name for path in user.iterdir()] == ["notes.txt"]
    earlier = project.root / ".bsmr-go-tools"
    assert not earlier.exists()
    earlier.mkdir()
    (earlier / ".bsmr-metadata.json").write_text(
        json.dumps({"generated_by": "bsmr go toolchain", "state": "acquired"})
    )
    for command in [("toolchain", "--check"), ("sync", "--check")]:
        result = project.run("go", *command)
        assert result.returncode != 0
        assert "run `bsmr go toolchain` to retire them" in result.stderr
    project.succeed("go", "toolchain")
    assert not earlier.exists()
    assert (user / "notes.txt").read_bytes() == b"user-owned\n"
    assert "Go SDK verified: 1.26.1" in project.succeed("go", "toolchain", "--check")
    project.succeed("go", "sync", "--check")


def test_running_daemon_reads_user_root_metadata(project: Project) -> None:
    """Evaluate from user-root metadata that a running daemon sees appear, break, and recover."""
    project.pin(directive="toolchain go1.26.1")
    shutil.copyfile(
        Path(__file__).parents[1] / "fixtures/go-action/main.go", project.root / "main.go"
    )
    project.succeed("go", "toolchain")
    project.succeed("go", "sync")
    metadata = project.home / ".bsmr/bootstrap/go/go1.26.1.json"
    recorded = metadata.read_bytes()
    build = ("build", "//:bin", "-c", "go.link_mode=internal")
    metadata.unlink()
    missing = project.run(*build)
    assert missing.returncode != 0
    assert "is missing; run `bsmr go toolchain`" in missing.stderr
    project.succeed("go", "toolchain")
    assert metadata.read_bytes() == recorded
    project.succeed(*build)
    metadata.write_text("{}")
    invalid = project.run(*build)
    assert invalid.returncode != 0
    assert "is invalid; run `bsmr go toolchain`" in invalid.stderr
    tampered = recorded.replace(b'"size": ', b'"size": 1')
    metadata.write_bytes(tampered)
    corrupt = project.run("go", "toolchain")
    assert corrupt.returncode != 0
    assert "failed to materialize the locked Go SDK" in corrupt.stderr
    assert metadata.read_bytes() == tampered
    metadata.write_bytes(recorded)
    project.succeed(*build)


def test_module_root_defines_the_toolchains_package(project: Project) -> None:
    """Build without the project lock when the module root itself holds no Go package."""
    project.pin(directive="toolchain go1.26.1")
    (project.root / "cmd").mkdir()
    shutil.copyfile(
        Path(__file__).parents[1] / "fixtures/go-action/main.go", project.root / "cmd/main.go"
    )
    project.succeed("go", "toolchain")
    project.succeed("go", "sync")
    outputs = json.loads(
        project.succeed(
            "build", "//cmd:bin", "-c", "go.link_mode=internal", "--show-full-json-output"
        )
    )
    result = subprocess.run([outputs["root//cmd:bin"]], capture_output=True, text=True, check=True)
    assert result.stdout.strip().split("-X:", 1)[0] == "go1.26.1"


def test_toolchain_commits_no_project_lock(project: Project) -> None:
    """Acquire without a project lock, and retire only an earlier release's marked lock."""
    project.pin(directive="toolchain go1.26.1")
    project.succeed("go", "toolchain")
    lock = project.root / ".bsmr-go-toolchain.json"
    assert not lock.exists()
    lock.write_text(json.dumps({"generated_by": "bsmr go toolchain", "schema": 1}))
    for command in [("toolchain", "--check"), ("sync", "--check")]:
        result = project.run("go", *command)
        assert result.returncode != 0
        assert "run `bsmr go toolchain` to retire them" in result.stderr
    project.succeed("go", "toolchain")
    assert not lock.exists()
    lock.write_text('{"note": "bsmr go toolchain"}\n')
    assert "Go SDK verified: 1.26.1" in project.succeed("go", "toolchain", "--check")
    project.succeed("go", "toolchain")
    assert lock.read_text() == '{"note": "bsmr go toolchain"}\n'


def test_sdk_is_an_action_input(project: Project) -> None:
    """Build and restore without a source-tree SDK or wrapper, then observe a changed SDK pin."""
    shutil.copyfile(
        Path(__file__).parents[1] / "fixtures/go-action/main.go", project.root / "main.go"
    )
    for version in ["1.26.1", "1.26.2"]:
        project.pin(directive=f"toolchain go{version}")
        project.succeed("go", "toolchain", "--version", version)
        project.succeed("go", "sync")
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
