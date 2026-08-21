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


import json
import os

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


@bsmr_test(inplace=False, data_dir="bxl/simple", allow_soft_errors=True)
async def test_bxl_fs_exists(bsmr: Bsmr) -> None:
    await bsmr.bxl("//bxl:fs.bxl:exists", "--", "--root_path", str(bsmr.cwd))


@bsmr_test(inplace=False, data_dir="bxl/simple", skip_for_os=["windows"])
async def test_bxl_fs_exists_symlink(bsmr: Bsmr) -> None:
    link_path = bsmr.cwd / "symlink/foo/bar"
    if not os.path.islink(link_path):
        os.unlink(link_path)
        os.symlink("../bar", link_path)
    await bsmr.bxl("//bxl:fs.bxl:exists_symlink")


@bsmr_test(inplace=False, data_dir="bxl/simple", allow_soft_errors=True)
async def test_bxl_fs_list(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl:fs.bxl:list_relative_path",
    )

    assert result.stdout.splitlines() == [
        "root//bin/TARGETS.fixture",
        "root//bin/kind",
    ]

    result = await bsmr.bxl(
        "//bxl:fs.bxl:list_absolute_path", "--", "--root_path", str(bsmr.cwd)
    )

    assert result.stdout.splitlines() == [
        "root//bin/TARGETS.fixture",
        "root//bin/kind",
    ]

    result = await bsmr.bxl(
        "//bxl:fs.bxl:list_source_artifact",
    )

    assert result.stdout.splitlines() == [
        "root//bin/kind/TARGETS.fixture",
        "root//bin/kind/rules.bzl",
    ]

    result = await bsmr.bxl(
        "//bxl:fs.bxl:list_file_node",
    )

    assert result.stdout.splitlines() == [
        "root//bin/kind/TARGETS.fixture",
        "root//bin/kind/rules.bzl",
    ]

    result = await bsmr.bxl(
        "//bxl:fs.bxl:list_dirs_only",
    )

    assert result.stdout.splitlines() == [
        "root//bin/kind",
    ]

    result = await bsmr.bxl("//bxl:fs.bxl:list_cell_path")

    expected_output = [
        "root//bin/TARGETS.fixture",
        "root//bin/kind",
    ]

    output = json.loads(result.stdout)
    assert output["@root//bin"] == expected_output
    assert output["root//bin"] == expected_output
    assert output["//bin"] == expected_output


@bsmr_test(inplace=False, data_dir="bxl/simple", allow_soft_errors=True)
async def test_bxl_fs_is_file(bsmr: Bsmr) -> None:
    await bsmr.bxl("//bxl:fs.bxl:is_file", "--", "--root_path", str(bsmr.cwd))


@bsmr_test(inplace=False, data_dir="bxl/simple", allow_soft_errors=True)
async def test_bxl_fs_is_dir(bsmr: Bsmr) -> None:
    await bsmr.bxl("//bxl:fs.bxl:is_dir", "--", "--root_path", str(bsmr.cwd))


@bsmr_test(
    inplace=False,
    data_dir="bxl/simple",
    skip_for_os=["windows"],
    allow_soft_errors=True,
)
async def test_bxl_fs_project_rel_path(bsmr: Bsmr) -> None:
    result = await bsmr.bxl("//bxl:fs.bxl:project_rel_path")

    assert result.stdout.splitlines() == [
        "bin/kind/TARGETS.fixture",
        "bin/kind/rules.bzl",
    ]


@bsmr_test(
    inplace=False,
    data_dir="bxl/simple",
    skip_for_os=["windows"],
    allow_soft_errors=True,
)
async def test_bxl_fs_abs_path_unsafe(bsmr: Bsmr) -> None:
    result = await bsmr.bxl("//bxl:fs.bxl:abs_path_unsafe")

    assert result.stdout.splitlines() == [
        str(bsmr.cwd / "bin/kind/TARGETS.fixture"),
        str(bsmr.cwd / "bin/kind/rules.bzl"),
    ]


@bsmr_test(inplace=False, data_dir="bxl/simple", skip_for_os=["windows"])
async def test_bxl_fs_source(bsmr: Bsmr) -> None:
    await bsmr.bxl("//bxl:fs.bxl:source")

    await expect_failure(
        bsmr.bxl("//bxl:fs.bxl:source_invalid_path"),
        stderr_regex="Inferred package path `root//fs` is not a valid package within the given file path `root//this/path/does/not/exist",
    )
    await expect_failure(
        bsmr.bxl("//bxl:fs.bxl:source_invalid_hint"),
        stderr_regex="Inferred package path `root//bin/kind` is not a valid package within the given file path `root//fs/src/source",
    )
    await expect_failure(
        bsmr.bxl("//bxl:fs.bxl:source_too_many_hints"),
        stderr_regex="Expected a single target hint, not an iterable",
    )


@bsmr_test(
    inplace=False,
    data_dir="bxl/simple",
    skip_for_os=["windows"],
    allow_soft_errors=True,
)
async def test_bxl_file_set_ops(bsmr: Bsmr) -> None:
    await bsmr.bxl("//bxl/fs.bxl:file_set_operations")
