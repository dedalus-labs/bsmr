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


@bsmr_test(data_dir="identifier")
async def test_analysis_action_ids_unique_identifier_within_category(
    bsmr: Bsmr,
) -> None:
    await expect_failure(
        bsmr.audit("providers", "//:yyy"),
        stderr_regex="Action category `foo` contains duplicate identifier `x`",
    )


@bsmr_test(data_dir="category")
async def test_analysis_action_ids_unique_singleton_category(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.audit("providers", "//:zzz"),
        stderr_regex="Analysis produced multiple actions with category `foo` and at least one of them had no identifier",
    )
