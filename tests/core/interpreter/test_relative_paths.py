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

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


@bsmr_test()
async def test_relative_path_basic(bsmr: Bsmr) -> None:
    assert "//foo/bar:test_basic" in (await bsmr.targets("//foo/bar:")).stdout


@bsmr_test()
async def test_relative_path_left_allowed_dir(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.targets("//foo/baz:"),
        stderr_regex="Relative import path `../../defs.bzl` is not allowed at the current location.",
    )


@bsmr_test()
async def test_relative_path_has_symlink(bsmr: Bsmr) -> None:
    os.symlink(bsmr.cwd, os.path.join(bsmr.cwd, "foo/sym"), target_is_directory=True)
    await expect_failure(
        bsmr.targets("//foo/sym/foo/bar:"),
        stderr_regex="Symlink found on the way from current dir `root//foo/sym/foo/bar` to allowed relative dir `root//foo`: `root//foo/sym`.",
    )


@bsmr_test()
async def test_relative_path_in_attribute_default_current(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.targets("//foo/default_current:target"),
        stderr_regex="Target pattern must be absolute",
    )


@bsmr_test()
async def test_relative_path_in_attribute_default_up(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.targets("//foo/default_up:target"),
        stderr_regex="Target pattern must be absolute",
    )
