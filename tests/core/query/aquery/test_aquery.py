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
from bsmr.tests.e2e_util.helper.golden import golden


@bsmr_test()
async def test_target(bsmr: Bsmr) -> None:
    stdout = (await bsmr.aquery("//:test", "-a", "identifier")).stdout

    golden(
        output=stdout,
        rel_path="target.golden.json",
    )


@bsmr_test()
async def test_all_outputs(bsmr: Bsmr) -> None:
    stdout = (await bsmr.aquery("all_outputs(//:test)", "-a", "identifier")).stdout

    golden(
        output=stdout,
        rel_path="all_outputs.golden.json",
    )


@bsmr_test()
async def test_all_actions(bsmr: Bsmr) -> None:
    stdout = (await bsmr.aquery("all_actions(//:test)", "-a", "identifier")).stdout

    golden(
        output=stdout,
        rel_path="all_actions.golden.json",
    )


@bsmr_test()
async def test_all_outputs_subtarget(bsmr: Bsmr) -> None:
    stdout = (
        await bsmr.aquery("all_outputs('//:test[sub]')", "-a", "identifier")
    ).stdout

    golden(
        output=stdout,
        rel_path="all_outputs_subtarget.golden.json",
    )


@bsmr_test()
async def test_filter(bsmr: Bsmr) -> None:
    stdout = (
        await bsmr.aquery(
            "attrfilter('identifier', 'other', all_actions('//:test[sub]'))",
            "-a",
            "identifier",
        )
    ).stdout

    golden(
        output=stdout,
        rel_path="filter.golden.json",
    )


@bsmr_test()
async def test_deps(bsmr: Bsmr) -> None:
    stdout = (await bsmr.aquery("deps(//:test)", "-a", "identifier")).stdout

    golden(
        output=stdout,
        rel_path="deps.golden.json",
    )


@bsmr_test()
async def test_bxl_aquery_target(bsmr: Bsmr) -> None:
    stdout = (await bsmr.bxl("//:aquery.bxl:target")).stdout
    golden(
        output=stdout,
        rel_path="bxl_target.golden.json",
    )


@bsmr_test()
async def test_bxl_aquery_all_outputs(bsmr: Bsmr) -> None:
    stdout = (await bsmr.bxl("//:aquery.bxl:all_outputs")).stdout

    golden(
        output=stdout,
        rel_path="bxl_all_outputs.golden.json",
    )


@bsmr_test()
async def test_bxl_aquery_all_actions(bsmr: Bsmr) -> None:
    stdout = (await bsmr.bxl("//:aquery.bxl:all_actions")).stdout

    golden(
        output=stdout,
        rel_path="bxl_all_actions.golden.json",
    )


@bsmr_test()
async def test_bxl_aquery_all_outputs_subtarget(bsmr: Bsmr) -> None:
    stdout = (await bsmr.bxl("//:aquery.bxl:all_outputs_subtarget")).stdout

    golden(
        output=stdout,
        rel_path="bxl_all_outputs_subtarget.golden.json",
    )


@bsmr_test()
async def test_bxl_aquery_attrfilter(bsmr: Bsmr) -> None:
    stdout = (await bsmr.bxl("//:aquery.bxl:attrfilter")).stdout

    golden(
        output=stdout,
        rel_path="bxl_filter.golden.json",
    )


@bsmr_test()
async def test_bxl_aquery_deps(bsmr: Bsmr) -> None:
    stdout = (await bsmr.bxl("//:aquery.bxl:deps")).stdout

    golden(
        output=stdout,
        rel_path="bxl_deps.golden.json",
    )


@bsmr_test()
async def test_bxl_aquery_eval(bsmr: Bsmr) -> None:
    stdout = (await bsmr.bxl("//:aquery.bxl:eval")).stdout

    golden(
        output=stdout,
        rel_path="bxl_eval.golden.json",
    )


@bsmr_test()
async def test_bxl_aquery_action_query_node(bsmr: Bsmr) -> None:
    await bsmr.bxl("//:aquery.bxl:action_query_node")
