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
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test, env


@bsmr_test()
@env(
    "BSMR_HARD_ERROR",
    "true",
)
async def test_missing_source_file_when_hard_errors_enabled(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.uquery("//package1:"),
        stderr_regex="Source file `non_existent_source_file.txt` does not exist as a member of package `prelude//package1`",
    )


@bsmr_test()
@env(
    "BSMR_HARD_ERROR",
    "false",
)
async def test_missing_source_file_when_hard_errors_disabled(bsmr: Bsmr) -> None:
    await bsmr.uquery("//package1:")
