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


import typing

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test
from bsmr.tests.e2e_util.helper.utils import filter_events


async def check_rule_type_names(
    bsmr: Bsmr, expected_rule_type_names: typing.List[typing.Optional[str]]
) -> None:
    rule_names = await filter_events(
        bsmr,
        "Result",
        "result",
        "build_response",
        "build_targets",
    )
    rule_names = rule_names[0]
    assert len(rule_names) == len(expected_rule_type_names)
    for actual, expected in zip(rule_names, expected_rule_type_names):
        if expected is not None:
            assert actual["target_rule_type_name"] == expected


@bsmr_test()
async def test_build_nested_subtargets(bsmr: Bsmr) -> None:
    await bsmr.build(
        "//:nested[sub][nested_sub]",
    )
    await check_rule_type_names(bsmr, ["nested_subtargets"])


@bsmr_test()
async def test_build_single_dep_touch(bsmr: Bsmr) -> None:
    await bsmr.build(
        "//:rule1",
    )
    await check_rule_type_names(bsmr, ["one"])


@bsmr_test()
async def test_build_two_out_of_order(bsmr: Bsmr) -> None:
    await bsmr.build(
        "//:rule1",
        "//:nested[sub][nested_sub]",
    )
    await check_rule_type_names(bsmr, ["nested_subtargets", "one"])


@bsmr_test()
async def test_build_rule_with_transition(bsmr: Bsmr) -> None:
    await bsmr.build(
        "//:a_writer_with_transition",
    )

    await check_rule_type_names(bsmr, ["three_with_transition"])


@bsmr_test()
async def test_build_all_in_target(bsmr: Bsmr) -> None:
    await bsmr.build(
        "//:",
    )
    await check_rule_type_names(
        bsmr,
        [
            "two",
            "three_with_transition",
            "nested_subtargets",
            "one",
            "one",
            "platform",
        ],
    )


@bsmr_test()
async def test_build_all_recursive(bsmr: Bsmr) -> None:
    await bsmr.build(
        "//...",
    )
    await check_rule_type_names(
        bsmr,
        [
            "two",
            "three_with_transition",
            "nested_subtargets",
            "one",
            "one",
            "platform",
        ],
    )
