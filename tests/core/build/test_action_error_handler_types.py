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

from bsmr.tests.e2e_util.api.buck import Buck
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.buck_workspace import buck_test


@buck_test()
async def test_action_error_handler_types(buck: Buck) -> None:
    await buck.bxl(
        "//:test_action_error_handler_types.bxl:test_action_error_handler_types"
    )


@buck_test()
async def test_output_when_no_error_handler_used(buck: Buck) -> None:
    failure = await expect_failure(
        buck.build("//:does_not_use_error_handler"),
    )

    assert "Action sub-errors produced by error handlers: <empty>" not in failure.stderr


@buck_test()
async def test_error_handler_succeed_on_nonetype(buck: Buck) -> None:
    await buck.build("//:error_handler_nonetype")


@buck_test()
async def test_output_for_error_handler_with_errorformat(buck: Buck) -> None:
    failure = await expect_failure(
        buck.build("//:error_handler_with_errorformat"),
    )

    assert "- [test_failure] main.rs:10 expected `;`, found `}`" in failure.stderr
    assert "manually created sub error" not in failure.stderr
