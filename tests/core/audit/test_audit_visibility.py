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


import pytest
from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


@bsmr_test()
@pytest.mark.parametrize(
    "rule, passes",
    [
        ("self//:pass1", True),
        ("self//:pass2", True),
        ("self//:pass3", True),
        ("self//:pass4", True),
        ("self//:fail1", False),
        ("self//:fail2", False),
        ("self//:fail3", False),
        ("self//:fail4", False),
        ("self//:fail5", False),
        ("self//:fail6", False),
    ],
)
async def test_audit_visibility(bsmr: Bsmr, rule: str, passes: bool) -> None:
    if passes:
        out = await bsmr.audit_visibility(rule)
        assert out.stdout == ""
    else:
        await expect_failure(
            bsmr.audit_visibility(rule),
            stderr_regex=f"not visible to `{rule}`",
        )
