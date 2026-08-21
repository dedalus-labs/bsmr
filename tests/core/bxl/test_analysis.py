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
async def test_bxl_analysis(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//analysis.bxl:providers_test",
    )

    lines = result.stdout.splitlines()
    assert "provides_foo_foo" in lines[0]
    assert "provides_foo_foo" in lines[1]

    result = await bsmr.bxl(
        "//analysis.bxl:dependency_test",
    )

    assert result.stdout.splitlines() == [
        "Dependency",
        "root//:stub (<unspecified>)",
    ]


@bsmr_test(write_invocation_record=True)
async def test_bxl_analysis_missing_subtarget(bsmr: Bsmr) -> None:
    res = await expect_failure(
        bsmr.bxl(
            "//analysis.bxl:missing_subtarget_test",
        ),
        stderr_regex="requested sub target named `missing_subtarget` .* is not available",
    )

    record = res.invocation_record()
    errors = record["errors"]

    assert len(errors) == 1
    assert errors[0]["category"] == "USER"


@bsmr_test()
async def test_bxl_analysis_unconfigured_target_error(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.bxl("//analysis.bxl:unconfigured_target_error_test"),
        stderr_regex="Type of parameter `labels` doesn't match",
    )
