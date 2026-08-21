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

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


@bsmr_test()
async def test_define_anon_bxl(bsmr: Bsmr) -> None:
    await bsmr.bxl(
        "//anon_bxl.bxl:define_anon",
    )


@bsmr_test()
async def test_define_wrong_type_anon_bxl(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.bxl("//wrong_type_anon_bxl.bxl:wrong_type"),
        stderr_regex="Type of parameter `impl` doesn't match,",
    )


@bsmr_test()
async def test_eval_anon_bxl(bsmr: Bsmr) -> None:
    await bsmr.bxl(
        "//anon_bxl.bxl:eval_anon_bxl",
    )


@bsmr_test()
async def test_check_anon_ouput_artifact(bsmr: Bsmr) -> None:
    await bsmr.bxl(
        "//anon_bxl.bxl:check_anon_ouput_artifact",
    )


@bsmr_test()
async def test_pass_string_to_arg_attr(bsmr: Bsmr) -> None:
    await bsmr.bxl("//anon_bxl.bxl:eval_of_anon_with_arg_bxl")


@bsmr_test()
async def test_content_based_output(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//anon_bxl.bxl:eval_of_anon_with_content_based_output_impl"
    )

    output_path = (bsmr.cwd / result.stdout.strip()).resolve()
    assert output_path.read_text() == "hello world"
