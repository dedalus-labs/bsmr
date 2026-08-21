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
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


@bsmr_test()
async def test_bxl_target_universe_keep_going_no_errors(bsmr: Bsmr) -> None:
    await bsmr.bxl(
        "//target_universe.bxl:target_universe_keep_going_no_errors",
    )


@bsmr_test()
async def test_bxl_target_universe_universe_target_set(bsmr: Bsmr) -> None:
    await bsmr.bxl(
        "//target_universe.bxl:target_universe_universe_target_set",
    )


@bsmr_test()
async def test_bxl_target_universe_keep_going_with_errors(bsmr: Bsmr) -> None:
    await bsmr.bxl(
        "//keep_going.bxl:target_universe_keep_going_with_errors",
    )


@bsmr_test()
async def test_bxl_target_universe_keep_going_list_input(bsmr: Bsmr) -> None:
    await bsmr.bxl(
        "//keep_going.bxl:target_universe_keep_going_list_input",
    )


@bsmr_test()
async def test_bxl_target_universe_keep_going_target_set_input(bsmr: Bsmr) -> None:
    await bsmr.bxl(
        "//keep_going.bxl:target_universe_keep_going_target_set_input",
    )


@bsmr_test()
async def test_bxl_target_universe_keep_going_mixed_list(bsmr: Bsmr) -> None:
    await bsmr.bxl(
        "//keep_going.bxl:target_universe_keep_going_mixed_list",
    )


@bsmr_test()
async def test_bxl_target_universe_keep_going_all_fail(bsmr: Bsmr) -> None:
    await bsmr.bxl(
        "//keep_going.bxl:target_universe_keep_going_all_fail",
    )


@bsmr_test()
async def test_bxl_target_universe_keep_going_incompatible_target_set(
    bsmr: Bsmr,
) -> None:
    result = await bsmr.bxl(
        "//keep_going.bxl:target_universe_keep_going_incompatible_target_set",
    )
    assert "Skipped 1 incompatible targets" in result.stderr
    assert "root//incompatible_targets:incompatible_target" in result.stderr


@bsmr_test()
async def test_bxl_target_universe_keep_going_incompatible_string_pattern(
    bsmr: Bsmr,
) -> None:
    result = await bsmr.bxl(
        "//keep_going.bxl:target_universe_keep_going_incompatible_string_pattern",
    )
    assert "Skipped 1 incompatible targets" in result.stderr
    assert "root//incompatible_targets:incompatible_target" in result.stderr


@bsmr_test()
async def test_bxl_target_universe_keep_going_incompatible_list(
    bsmr: Bsmr,
) -> None:
    result = await bsmr.bxl(
        "//keep_going.bxl:target_universe_keep_going_incompatible_list",
    )
    assert "Skipped 1 incompatible targets" in result.stderr
    assert "root//incompatible_targets:incompatible_target" in result.stderr
