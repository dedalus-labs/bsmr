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
import re

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


def _replace_hash(s: str) -> str:
    return re.sub(r"\b[0-9a-f]{16}\b", "<HASH>", s)


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_bxl_new_target_set(bsmr: Bsmr) -> None:
    await bsmr.bxl(
        "//bxl/new_target_set.bxl:new_ctarget_set",
    )

    await bsmr.bxl(
        "//bxl/new_target_set.bxl:new_utarget_set",
    )


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_bxl_target_set_ops(bsmr: Bsmr) -> None:
    await bsmr.bxl("//bxl/target_set_ops.bxl:test_operations")


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_bxl_target_platform_from_value_as_starlark_target_label(
    bsmr: Bsmr,
) -> None:
    # Pass in explicit target platform from client. Result should be configured with this target platform.
    result = await bsmr.bxl(
        "--target-platforms",
        "root//platforms:platform2",
        "//bxl/cquery.bxl:owner_test",
    )
    assert (
        _replace_hash(result.stdout)
        == "[root//bin:the_binary (root//platforms:platform2#<HASH>)]\n"
    )

    # No target platform specified from client context. Result should be configured with root//platforms:platform1
    result = await bsmr.bxl(
        "//bxl/cquery.bxl:owner_test",
    )
    assert (
        _replace_hash(result.stdout)
        == "[root//bin:the_binary (root//platforms:platform1#<HASH>)]\n"
    )

    # Target platform from client context should be overridden by what's declared in cquery.
    result = await bsmr.bxl(
        "--target-platforms",
        "root//platforms:platform2",
        "//bxl/cquery.bxl:owner_test_with_target_platform",
    )
    assert (
        _replace_hash(result.stdout)
        == "[root//bin:the_binary (root//platforms:platform1#<HASH>)]\n"
    )


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_bxl_unconfigured_sub_targets(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl/providers.bxl:unconfigured_sub_targets",
    )

    output = json.loads(result.stdout.strip())
    assert output["lib1"] == "root//lib:lib1"
    assert output["lib1_FooInfo"] == "root//lib:lib1[FooInfo]"
    assert output["lib2"] == "root//lib:lib2"
    assert output["lib3_FooInfo"] == "root//lib:lib3[FooInfo]"


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_bxl_target_exists(bsmr: Bsmr) -> None:
    await bsmr.bxl(
        "//bxl/target_exists.bxl:target_exists",
    )

    await expect_failure(
        bsmr.bxl("//bxl/target_exists.bxl:target_exists_no_target_patterns"),
        stderr_regex="Expected a single target as a string literal, not a target pattern",
    )

    await expect_failure(
        bsmr.bxl("//bxl/target_exists.bxl:target_exists_no_subtargets"),
        stderr_regex="Expecting target pattern, without providers",
    )
