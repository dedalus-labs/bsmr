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
async def test_action_error_handler_types(bsmr: Bsmr) -> None:
    await bsmr.bxl(
        "//:test_action_error_handler_types.bxl:test_action_error_handler_types"
    )


@bsmr_test()
async def test_output_when_no_error_handler_used(bsmr: Bsmr) -> None:
    failure = await expect_failure(
        bsmr.build("//:does_not_use_error_handler"),
    )

    assert "Action sub-errors produced by error handlers: <empty>" not in failure.stderr


@bsmr_test()
async def test_error_handler_succeed_on_nonetype(bsmr: Bsmr) -> None:
    await bsmr.build("//:error_handler_nonetype")


@bsmr_test()
async def test_output_for_error_handler_with_errorformat(bsmr: Bsmr) -> None:
    failure = await expect_failure(
        bsmr.build("//:error_handler_with_errorformat"),
    )

    assert "- [test_failure] main.rs:10 expected `;`, found `}`" in failure.stderr
    assert "manually created sub error" not in failure.stderr
