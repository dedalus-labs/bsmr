# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

"""Check a shared directory artifact once per command and still repair it between commands."""

import os
import re
import shutil
import subprocess
from collections import Counter
from collections.abc import Iterator
from pathlib import Path

import pytest

CONSUMERS = 16
FILES = 2000
ISOLATION = "presence"
FILE_ENTRY = re.compile(r'"([^"]*/__tree__/tree/d\d+/f\d+)"')


class Project:
    """Drive the real binary in a copy of the presence fixture with its own HOME."""

    def __init__(self, root: Path) -> None:
        """Copy the fixture and isolate the action cache below a fresh HOME."""
        self.binary = Path(os.environ["BSMR_BINARY"]).resolve(strict=True)
        self.root = root / "project"
        home = root / "home"
        home.mkdir()
        shutil.copytree(Path(__file__).parents[1] / "fixtures" / "presence", self.root)
        self.env = os.environ | {"HOME": str(home)}
        self.env.pop("XDG_CACHE_HOME", None)

    def command(self, *args: str) -> list[str]:
        """Return a command line for the test's isolation."""
        return [str(self.binary), "--isolation-dir", ISOLATION, *args]

    def run(self, *args: str, succeed: bool = True) -> str:
        """Run one command, require the expected outcome, and return its combined output."""
        result = subprocess.run(
            self.command(*args), cwd=self.root, env=self.env, capture_output=True, text=True
        )
        output = result.stdout + result.stderr
        assert (result.returncode == 0) == succeed, output
        return output

    def tree(self) -> Path:
        """Return the materialized directory artifact."""
        (tree,) = (self.root / "bsmr-out" / ISOLATION / "art").glob("root/*/__tree__/tree")
        return tree


@pytest.fixture
def project(tmp_path: Path) -> Iterator[Project]:
    """Stop the test's daemon even when an assertion fails."""
    result = Project(tmp_path)
    try:
        yield result
    finally:
        result.run("kill")


def test_shared_tree_checked_once(project: Project, tmp_path: Path) -> None:
    """Consumers of one directory artifact must not each stat every entry again.

    The in-process daemon runs under strace, so every stat of a tree entry is
    counted. A presence check per consumer stats each file once per consumer.
    """
    strace = shutil.which("strace")
    assert strace, "this test requires strace"
    trace = tmp_path / "stat.trace"
    targets = [f"//:use{index}" for index in range(CONSUMERS)]
    command = project.command("--no-daemon", "build", *targets)
    syscalls = "trace=statx,newfstatat,lstat,stat"
    subprocess.run(
        [strace, "-f", "-qq", "-e", syscalls, "-e", "signal=none", "-o", trace, *command],
        cwd=project.root,
        env=project.env,
        check=True,
    )
    stats = Counter(match[1] for match in FILE_ENTRY.finditer(trace.read_text()))
    assert len(stats) == FILES, f"expected every file to be checked, saw {len(stats)}"
    path, count = stats.most_common(1)[0]
    assert count < CONSUMERS, f"{path} was statted {count} times by {CONSUMERS} consumers"


def test_tree_repaired_between_commands(project: Project) -> None:
    """Each command re-checks the tree, so a deleted file is restored and a wrong type fails."""
    project.run("build", "//:use0")
    project.run("clean")
    project.run("build", "//:use0")
    missing = project.tree() / "d3" / "f7"
    missing.unlink()
    project.run("build", "//:use1")
    assert missing.read_text() == "7\n"
    project.run("kill")
    missing.unlink()
    project.run("build", "//:use2")
    assert missing.read_text() == "7\n"
    missing.unlink()
    missing.mkdir()
    output = project.run("build", "//:use3", succeed=False)
    assert "has the wrong file type" in output
