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


import fileinput
import os
from pathlib import Path

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test
from bsmr.tests.e2e_util.helper.utils import random_string


@bsmr_test(data_dir="modify")
async def test_modify_genrule(bsmr: Bsmr) -> None:
    result = await bsmr.build("//:writer")
    output = result.get_build_report().output_for_target("root//:writer")
    assert Path(output).read_text() == "HELLO\n"

    # Change "HELLO" in TARGETS to "GOODBYE"
    with fileinput.input(bsmr.cwd / "TARGETS.fixture", inplace=True) as f:
        for line in f:
            print(line.replace("HELLO", "GOODBYE"), end="")

    result = await bsmr.build("//:writer")
    output = result.get_build_report().output_for_target("root//:writer")
    assert Path(output).read_text() == "GOODBYE\n"


@bsmr_test(data_dir="modify")
async def test_modify_src(bsmr: Bsmr) -> None:
    result = await bsmr.build("//:mysrcrule")
    output = result.get_build_report().output_for_target("root//:mysrcrule")
    assert Path(output).read_text() == "HELLO\n"

    (bsmr.cwd / "src.txt").write_text("GOODBYE\n")
    result = await bsmr.build("//:mysrcrule")
    output = result.get_build_report().output_for_target("root//:mysrcrule")
    assert Path(output).read_text() == "GOODBYE\n"


@bsmr_test(data_dir="modify")
async def test_modify_genrule_notify(bsmr: Bsmr) -> None:
    with open(bsmr.cwd / ".bsmr", "a") as bsmrconfig:
        bsmrconfig.write("\n[bsmr]\nfile_watcher = notify")
    await bsmr.kill()  # Ensure the config gets picked up
    await test_modify_genrule(bsmr)


@bsmr_test(data_dir="modify")
async def test_notify_observes_rapid_source_edits(bsmr: Bsmr) -> None:
    """Require every build to observe the source state present at invocation."""
    with open(bsmr.cwd / ".bsmr", "a") as bsmrconfig:
        bsmrconfig.write("\n[bsmr]\nfile_watcher = notify")
    await bsmr.kill()

    source = bsmr.cwd / "src.txt"
    for revision in range(128):
        expected = f"VALUE-{revision:08x}\n"
        source.write_text(expected)
        result = await bsmr.build("//:mysrcrule")
        output = result.get_build_report().output_for_target("root//:mysrcrule")
        assert Path(output).read_text() == expected


@bsmr_test(data_dir="modify")
async def test_deleted_materialized_output_is_restored(bsmr: Bsmr) -> None:
    """Restore a deleted output from its retained content-addressed recipe."""
    result = await bsmr.build("//:mysrcrule")
    output = result.get_build_report().output_for_target("root//:mysrcrule")
    Path(output).unlink()

    result = await bsmr.build("//:mysrcrule")
    output = result.get_build_report().output_for_target("root//:mysrcrule")
    assert Path(output).read_text() == "HELLO\n"


@bsmr_test(data_dir="modify")
async def test_modify_directory(bsmr: Bsmr) -> None:
    # Test for the bug reported in T99593442
    os.mkdir(bsmr.cwd / "a_dir")
    with open(bsmr.cwd / "a_dir" / "test.txt", "w") as file:
        file.write("test")
    await bsmr.build("//:writer")
    # Remove a directory, and change a file, so the file gets spotted,
    # and we'd better note that the directory no longer exists
    os.remove(bsmr.cwd / "a_dir" / "test.txt")
    os.rmdir(bsmr.cwd / "a_dir")
    await bsmr.build("//:writer")


@bsmr_test(data_dir="modify_file_during_build")
async def test_modify_file_during_build(bsmr: Bsmr) -> None:
    # We need to write some random stuff to the file first so that Bsmr will
    # have to attempt to upload it to RE (which will fail because by that time
    # we will have overwritten it with other content).
    with open(bsmr.cwd / "text", "w", encoding="utf-8") as f:
        f.write(random_string())

    await expect_failure(
        bsmr.build("//:check"),
        stderr_regex="modified files while the build was in progress",
    )


@bsmr_test(data_dir="modify_file_during_build")
async def test_file_notify(bsmr: Bsmr) -> None:
    # We need to write some random stuff to the file first so that Bsmr will
    # have to attempt to upload it to RE (which will fail because by that time
    # we will have overwritten it with other content).
    with open(bsmr.cwd / "text", "w", encoding="utf-8") as f:
        f.write(random_string())

    await expect_failure(
        bsmr.build("//:check"),
        stderr_regex="modified files while the build was in progress",
    )
