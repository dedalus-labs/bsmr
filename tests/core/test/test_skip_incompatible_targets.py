#!/usr/bin/env fbpython
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
    "BSMR_ALLOW_INTERNAL_TEST_RUNNER_DO_NOT_USE", "1"
)  # needed to avoid failure on missing bsmr-tpx in bsmr-out
async def test_test_skip_incompatible_targets(bsmr: Bsmr) -> None:
    targetA = "root//:compatible-with-A"
    targetB = "root//:compatible-with-B"
    platformA = "root//:platA"

    await expect_failure(
        bsmr.test(
            targetA,
            targetB,
            f"--target-platforms={platformA}",
            test_executor="",
        ),
        stderr_regex=rf"{targetB}\s*is incompatible with {platformA}#.*$",
    )

    result = await bsmr.test(
        targetA,
        targetB,
        f"--target-platforms={platformA}",
        "--skip-incompatible-targets",
        test_executor="",
    )
    assert targetA in result.stderr
    assert targetB not in result.stderr

    result.check_returncode()
