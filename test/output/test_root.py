# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

"""Locate isolation output directories through the real CLI without starting a daemon."""

import os
import shutil
import subprocess
from pathlib import Path

import pytest


class Project:
    """Own an isolated CLI environment and a project copied from static inputs."""

    def __init__(self, root: Path) -> None:
        """Copy the project and isolate HOME without changing the selected binary."""
        self.binary = Path(os.environ["BSMR_BINARY"]).resolve(strict=True)
        self.root = root / "project"
        self.home = root / "home"
        self.home.mkdir()
        shutil.copytree(Path(__file__).parents[1] / "fixtures" / "output-root", self.root)
        self.nested = self.root / "nested"
        self.nested.mkdir()
        self.alias = root / "alias"
        self.alias.symlink_to(self.nested, target_is_directory=True)
        self.env = os.environ | {"HOME": str(self.home)}
        self.env.pop("BSMR_ISOLATION_DIR", None)

    def query(self, cwd: Path, isolation: str, directory: Path | None = None) -> Path:
        """Return the CLI's output directory and retain diagnostics on failure."""
        args = [str(self.binary), "--isolation-dir", isolation, "root", "--kind", "output"]
        if directory is not None:
            args.extend(["--dir", str(directory)])
        result = subprocess.run(args, cwd=cwd, env=self.env, capture_output=True, text=True)
        assert result.returncode == 0, result.stdout + result.stderr
        output = Path(result.stdout.strip())
        return output


@pytest.fixture
def project(tmp_path: Path) -> Project:
    """Give each test its own source tree, HOME and symlink alias."""
    result = Project(root=tmp_path)
    return result


@pytest.mark.parametrize("isolation", ["default", "alternate"])
@pytest.mark.parametrize("location", ["root", "nested", "alias", "directory"])
def test_output_root(project: Project, isolation: str, location: str) -> None:
    """Every invocation path names the canonical isolation without creating state."""
    if location == "directory":
        output = project.query(
            cwd=project.root.parent, isolation=isolation, directory=project.alias
        )
    else:
        cwd = {"root": project.root, "nested": project.nested, "alias": project.alias}[location]
        output = project.query(cwd=cwd, isolation=isolation)
    assert output == project.root / "bsmr-out" / isolation
    assert not output.exists()
    assert list(project.home.iterdir()) == []
    assert sorted(path.name for path in project.root.iterdir()) == [".bsmr", "nested"]


def test_directory_selects_another_project(project: Project) -> None:
    """An explicit directory selects its own project rather than the caller's project."""
    other = project.root.parent / "other"
    shutil.copytree(project.root, other)
    output = project.query(cwd=project.nested, isolation="selected", directory=other / "nested")
    assert output == other / "bsmr-out" / "selected"
    assert not output.exists()
    assert list(project.home.iterdir()) == []
