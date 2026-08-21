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


@bsmr_test(skip_for_os=["windows"])
async def test_run_action_timeout_expires(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.build("//:slow_with_timeout"),
        stderr_regex="timed out after",
    )


@bsmr_test(skip_for_os=["windows"])
async def test_run_action_timeout_succeeds(bsmr: Bsmr) -> None:
    result = await bsmr.build("//:fast_with_timeout")
    output = result.get_build_report().output_for_target("//:fast_with_timeout")
    assert output.read_text().strip() == "hello"
