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

# Empty test executor forces internal test executor to be used.
INTERNAL_TEST_EXECUTOR = ""


@bsmr_test()
@env("BSMR_ALLOW_INTERNAL_TEST_RUNNER_DO_NOT_USE", "1")
async def test_internal_test_executor(bsmr: Bsmr) -> None:
    await bsmr.test(
        ":trivial_pass",
        test_executor=INTERNAL_TEST_EXECUTOR,
    )


@bsmr_test()
@env("TEST_VAR", "BAD_VALUE")
@env("BSMR_ALLOW_INTERNAL_TEST_RUNNER_DO_NOT_USE", "1")
async def test_internal_test_executor_env(bsmr: Bsmr) -> None:
    await bsmr.test(
        ":check_env",
        "--",
        "--env",
        "TEST_VAR=TEST_VALUE",
        test_executor=INTERNAL_TEST_EXECUTOR,
    )


@bsmr_test()
@env("BSMR_ALLOW_INTERNAL_TEST_RUNNER_DO_NOT_USE", "1")
async def test_internal_test_executor_timeout(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.test(
            ":timeout",
            "--",
            "--timeout",
            "1",
            test_executor=INTERNAL_TEST_EXECUTOR,
        ),
        stderr_regex="Timeout: ",
    )
