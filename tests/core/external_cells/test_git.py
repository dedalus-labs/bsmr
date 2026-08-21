# ===----------------------------------------------------------------------===
# Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc
# Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

# pyre-strict

import os
import shutil
import stat
import subprocess
import sys
import typing
from pathlib import Path

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


def _repo(cwd: Path) -> Path:
    return (cwd.parent / "external").absolute()


def _git(args: list[str], cwd: Path) -> str:
    print("Running " + " ".join(args))
    out = subprocess.check_output(["git"] + args, cwd=_repo(cwd)).decode().strip()
    print(f"Git output: {out}")
    return out


def _git_commit(cwd: Path) -> str:
    _git(["add", "."], cwd=cwd)
    _git(["commit", "-m", "Commit name"], cwd=cwd)
    return _git(["log", "--format=format:%H%n", "-1", "-r", "."], cwd=cwd)


def _set_revision(rev: str, cwd: Path) -> None:
    p = cwd / ".bsmr"
    data = p.read_text().splitlines()[:-2]
    data.append(f"  git_origin = file://{_repo(cwd)}")
    data.append(f"  commit_hash = {rev}")
    p.write_text("\n".join(data))


def _init_repo(cwd: Path) -> None:
    _repo(cwd).mkdir(parents=True, exist_ok=True)
    _git(["init"], cwd=cwd)
    _git(["config", "user.name", "notarealuser"], cwd=cwd)
    _git(["config", "user.email", "notarealuser@fb.com"], cwd=cwd)
    shutil.copytree(cwd / "template", _repo(cwd), dirs_exist_ok=True)
    rev = _git_commit(cwd=cwd)
    _set_revision(rev, cwd=cwd)


@bsmr_test()
async def test_expand_external(bsmr: Bsmr) -> None:
    _init_repo(cwd=bsmr.cwd)
    await bsmr.expand_external_cell("libfoo")
    assert (bsmr.cwd / "libfoo" / "src.txt").exists()
    assert "buildfile" in (bsmr.cwd / "libfoo" / ".bsmr").read_text()


@bsmr_test()
async def test_non_master_ancestor(bsmr: Bsmr) -> None:
    _init_repo(cwd=bsmr.cwd)

    _git(["switch", "-c", "other"], cwd=bsmr.cwd)
    (_repo(cwd=bsmr.cwd) / "src.txt").write_text("change")
    rev = _git_commit(cwd=bsmr.cwd)
    _set_revision(rev, cwd=bsmr.cwd)

    _git(["switch", "master"], cwd=bsmr.cwd)
    (_repo(cwd=bsmr.cwd) / "src.txt").write_text("change2")
    _git_commit(cwd=bsmr.cwd)

    res = await bsmr.build_without_report("libfoo//:t", "--show-full-simple-output")
    assert Path(res.stdout.strip()).read_text().strip() == "change"


@bsmr_test()
async def test_changing_commit(bsmr: Bsmr) -> None:
    _init_repo(cwd=bsmr.cwd)

    res = await bsmr.build_without_report("libfoo//:t", "--show-full-simple-output")
    assert Path(res.stdout.strip()).read_text().strip() == ""

    (_repo(cwd=bsmr.cwd) / "src.txt").write_text("change")
    rev = _git_commit(cwd=bsmr.cwd)
    _set_revision(rev, cwd=bsmr.cwd)

    res = await bsmr.build_without_report("libfoo//:t", "--show-full-simple-output")
    assert Path(res.stdout.strip()).read_text().strip() == "change"


@bsmr_test()
async def test_full_clean_cycle(bsmr: Bsmr) -> None:
    _init_repo(cwd=bsmr.cwd)

    res = await bsmr.build_without_report(
        "libfoo//:t[src]", "--show-full-simple-output"
    )
    src_path = Path(res.stdout.strip())

    await bsmr.clean()

    assert not Path(src_path).exists()


@bsmr_test()
async def test_noop_commit_change_causes_rebuild(bsmr: Bsmr) -> None:
    """Changing the commit hash of a git external cell causes a full rebuild
    even when the file contents are identical. This test uses a non-deterministic
    action to detect whether a rebuild occurred: if the outputs differ, the
    action was re-executed rather than cached.

    When external cell caching is fixed to be content-based, this assertion
    should flip to `output1 == output2`."""
    _init_repo(cwd=bsmr.cwd)

    res1 = await bsmr.build_without_report(
        "libfoo//:nondeterministic", "--show-full-simple-output", "--local-only"
    )
    output1 = Path(res1.stdout.strip()).read_text().strip()

    # New commit that adds an unrelated file (no build-relevant changes)
    (_repo(cwd=bsmr.cwd) / "README.md").write_text("unrelated file")
    rev = _git_commit(cwd=bsmr.cwd)
    _set_revision(rev, cwd=bsmr.cwd)

    res2 = await bsmr.build_without_report(
        "libfoo//:nondeterministic", "--show-full-simple-output", "--local-only"
    )
    output2 = Path(res2.stdout.strip()).read_text().strip()

    # Currently rebuilds (different output). After fix: should be cached (same output).
    assert output1 != output2


@bsmr_test()
async def test_no_refetch_on_restart(bsmr: Bsmr) -> None:
    _init_repo(cwd=bsmr.cwd)

    await bsmr.build("libfoo//:t")
    await bsmr.kill()

    def _remove_readonly(
        func: typing.Callable[..., object], path: str, _exc: object
    ) -> None:
        os.chmod(path, stat.S_IWRITE)
        func(path)

    shutil.rmtree(
        _repo(cwd=bsmr.cwd),
        onexc=_remove_readonly if sys.platform == "win32" else None,
    )
    await bsmr.build("libfoo//:t")
