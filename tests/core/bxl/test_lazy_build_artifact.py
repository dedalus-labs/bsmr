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
async def test_build_artifact(bsmr: Bsmr) -> None:
    res = await bsmr.bxl(
        "//:lazy_build_artifact.bxl:build_artifact",
    )
    assert "foo.txt" in res.stdout
    assert "bar.txt" in res.stdout


@bsmr_test()
async def test_build_artifact_catch_error(bsmr: Bsmr) -> None:
    res = await bsmr.bxl(
        "//:lazy_build_artifact.bxl:build_artifact_fail",
    )
    assert "foo.txt" in res.stdout


@bsmr_test()
async def test_cannot_build_dynmiac_action_output(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.bxl(
            "//:lazy_build_artifact.bxl:dynamic",
        ),
        stderr_regex="does not accept declared artifact",
    )


@bsmr_test()
async def test_cannot_bxl_action_output(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.bxl(
            "//:lazy_build_artifact.bxl:bxl_action_output",
        ),
        stderr_regex="does not accept declared artifact",
    )
