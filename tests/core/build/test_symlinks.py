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
import tempfile
from pathlib import Path

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test
from bsmr.tests.e2e_util.helper.utils import expect_exec_count


def setup_symlink(symlink_path: Path, target: Path) -> None:
    symlink_path.parent.mkdir(parents=True, exist_ok=True)

    if not os.path.islink(symlink_path) and os.path.isdir(symlink_path):
        shutil.rmtree(symlink_path)
    else:
        symlink_path.unlink(missing_ok=True)

    os.symlink(target, symlink_path)


@bsmr_test(extra_bsmr_config={"bsmr": {"use_correct_source_symlink_reading": "true"}})
async def test_symlink_target_tracked_for_rebuild(bsmr: Bsmr) -> None:
    setup_symlink(bsmr.cwd / "src" / "link", Path("../dir"))

    await bsmr.build("//:cp")
    await expect_exec_count(bsmr, 1)

    await bsmr.build("//:cp")
    await expect_exec_count(bsmr, 0)

    with open(bsmr.cwd / "dir/file", "w") as file:
        file.write("GOODBYE\n")

    # This isn't really behavior  we want to guarantee and we'd rather users
    # don't use symlinks, but this is very observable (and it's not worse than
    # just reading the files then pretending they are never used!)
    await bsmr.build("//:cp")
    await expect_exec_count(bsmr, 1)


@bsmr_test(
    setup_eden=True,
    extra_bsmr_config={"bsmr": {"use_correct_source_symlink_reading": "true"}},
)
async def test_symlinks_redirection(bsmr: Bsmr) -> None:
    setup_symlink(bsmr.cwd / "src" / "link", Path("../dir"))

    await bsmr.build("//:cp")
    await expect_exec_count(bsmr, 1)

    await bsmr.build("//:cp")
    await expect_exec_count(bsmr, 0)

    # We change the symlink which should invalidate all files depending on it
    setup_symlink(bsmr.cwd / "src" / "link", Path("../dir2"))

    await bsmr.build("//:cp")
    await expect_exec_count(bsmr, 1)


@bsmr_test(
    setup_eden=True,
    extra_bsmr_config={"bsmr": {"use_correct_source_symlink_reading": "true"}},
)
async def test_symlinks_external(bsmr: Bsmr) -> None:
    top_level = Path(tempfile.mkdtemp())

    (top_level / "nested1").mkdir()
    (top_level / "nested2").mkdir()
    (top_level / "nested1" / "file").write_text("HELLO")
    (top_level / "nested2" / "file").write_text("GOODBYE")

    setup_symlink(bsmr.cwd / "ext" / "link", top_level / "nested1")

    await bsmr.build("//:ext")
    await expect_exec_count(bsmr, 1)

    await bsmr.build("//:ext")
    await expect_exec_count(bsmr, 0)

    setup_symlink(bsmr.cwd / "ext" / "link", top_level / "nested2")

    await bsmr.build("//:ext")
    await expect_exec_count(bsmr, 1)


@bsmr_test(extra_bsmr_config={"bsmr": {"use_correct_source_symlink_reading": "true"}})
async def test_no_read_through_symlinks(bsmr: Bsmr) -> None:
    res = await bsmr.build_without_report(
        "//:stat_symlink",
        "--out",
        "-",
        "--remote-only",
    )
    # Just check that we don't always return `True`
    assert res.stdout.strip() == "False"

    setup_symlink(bsmr.cwd / "src" / "link", Path("..") / "dir")

    res = await bsmr.build_without_report(
        "//:stat_symlink",
        "--out",
        "-",
        "--remote-only",
    )
    assert res.stdout.strip() == "True"

    res = await bsmr.build_without_report(
        "//:stat_symlink_in_dir",
        "--out",
        "-",
        "--remote-only",
    )
    assert res.stdout.strip() == "True"


@bsmr_test(extra_bsmr_config={"bsmr": {"use_correct_source_symlink_reading": "true"}})
async def test_no_read_through_source_symlinks_to_file(bsmr: Bsmr) -> None:
    res = await bsmr.build_without_report(
        "//:stat_symlink",
        "--out",
        "-",
        "--remote-only",
    )
    # Just check that we don't always return `True`
    assert res.stdout.strip() == "False"

    setup_symlink(
        bsmr.cwd / "src" / "link",
        Path("..") / "dir" / "file",
    )

    res = await bsmr.build_without_report(
        "//:stat_symlink",
        "--out",
        "-",
        "--remote-only",
    )
    assert res.stdout.strip() == "True"


@bsmr_test(extra_bsmr_config={"bsmr": {"use_correct_source_symlink_reading": "true"}})
async def test_no_read_through_source_symlinks_to_in_symlink_target(bsmr: Bsmr) -> None:
    for s in ("dir", "dir2/dir"):
        (bsmr.cwd / s).mkdir(parents=True, exist_ok=True)
        (bsmr.cwd / s / "file").write_text(s)
    setup_symlink(bsmr.cwd / "redirectvia", Path("dir2") / "dir")

    setup_symlink(
        bsmr.cwd / "src" / "link",
        Path("..") / "redirectvia" / ".." / "dir" / "file",
    )

    res = await bsmr.build_without_report(
        "//:cp_src_link_via_builtin",
        "--out",
        "-",
    )
    # FIXME(JakobDegen): Should be `dir2/dir`. The fact that `redirectvia`, found in the symlink
    # target, is itself a symlink is completely ignored
    assert res.stdout.strip() == "dir"


@bsmr_test(setup_eden=True)
async def test_eden_io_read_symlink_dir_build_target(bsmr: Bsmr) -> None:
    setup_symlink(bsmr.cwd / "testlink", bsmr.cwd / "symdir" / "dir")

    await bsmr.build("//:symlink_dep")


@bsmr_test(setup_eden=True)
async def test_eden_io_read_symlink_dir_list_target(bsmr: Bsmr) -> None:
    setup_symlink(bsmr.cwd / "testlink", bsmr.cwd / "symdir")

    await bsmr.targets("//testlink/dir:")
