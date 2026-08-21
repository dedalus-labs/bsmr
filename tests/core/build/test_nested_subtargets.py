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


from bsmr.tests.e2e_util import asserts
from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


@bsmr_test()
async def test_build_nested_subtargets(bsmr: Bsmr) -> None:
    result = await bsmr.build(
        "//:nested[sub][nested_sub]",
    )
    build_report = result.get_build_report()

    output = build_report.output_for_target("//:nested", "sub|nested_sub")

    assert output.read_text().rstrip() == "foo_content"
    asserts.assert_not_executable(output)


@bsmr_test()
async def test_build_nested_subtargets_errors(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.build(
            "//:nested[bad]",
        ),
        stderr_regex="Available subtargets are.*sub",
    )

    await expect_failure(
        bsmr.build(
            "//:nested[sub][bad]",
        ),
        stderr_regex="Available subtargets are.*nested_sub",
    )
