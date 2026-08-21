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


import platform

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


@bsmr_test()
async def test_run_with_source_macros(bsmr: Bsmr) -> None:
    sep = "\\" if platform.system() == "Windows" else "/"
    result = await bsmr.run("//source:echo_file")
    assert result.stdout.endswith(f"source{sep}foo.txt\n")

    result = await bsmr.run("//source:echo_dir")
    assert result.stdout.endswith(f"source{sep}bar\n")

    result = await bsmr.run("//source:cat_file")
    assert result.stdout == "foo file\n"

    result = await bsmr.run("//source:cat_dir")
    assert result.stdout == "bar file\n"


@bsmr_test()
async def test_no_dep_in_source(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.build("//dep_as_source:uses_dep"),
        stderr_regex="Source file `:trivial` does not exist",
    )
