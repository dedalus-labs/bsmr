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
async def test_validation_affects_build_command(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.build(":plate"),
        stderr_regex="""
Validation for `prelude//:mate \\(<unspecified>\\)` failed:

Here I am describing the failure reason

Full validation result is located at""",
    )
    await bsmr.build(":date")


@bsmr_test(write_invocation_record=True)
async def test_validation_affects_run_command(bsmr: Bsmr) -> None:
    res = await expect_failure(
        bsmr.run(
            ":plate",
        ),
        stderr_regex="""
Validation for `prelude//:mate \\(<unspecified>\\)` failed:

Here I am describing the failure reason

Full validation result is located at""",
    )

    record = res.invocation_record()
    assert len(record["errors"]) == 1

    await bsmr.run(":date")


@bsmr_test(write_invocation_record=True)
@env("BSMR_ALLOW_INTERNAL_TEST_RUNNER_DO_NOT_USE", "1")
async def test_validation_affects_test_command(bsmr: Bsmr) -> None:
    res = await expect_failure(
        bsmr.test(
            ":plate",
            test_executor="",
        ),
        stderr_regex="""
Validation for `prelude//:mate \\(<unspecified>\\)` failed:

Here I am describing the failure reason

Full validation result is located at""",
    )

    record = res.invocation_record()
    assert len(record["errors"]) == 1

    await bsmr.test(":date", test_executor="")


@bsmr_test(write_invocation_record=True)
async def test_validation_affects_install_command(bsmr: Bsmr) -> None:
    res = await expect_failure(
        bsmr.install(
            ":plate",
        ),
        stderr_regex="Validation for `prelude//:mate \\(<unspecified>\\)` failed",
    )

    record = res.invocation_record()
    assert len(record["errors"]) == 1

    # It's too complicated to set up installer properly.
    # We intentionally fail on the installer side, but interpret
    # an attempt to run it as a successful verification.
    res = await expect_failure(
        bsmr.install(
            ":date",
        ),
        stderr_regex="Installer: Incoming connection accepted, now closing it",
    )

    record = res.invocation_record()
    assert len(record["errors"]) == 1


@bsmr_test()
async def test_optional_validation(bsmr: Bsmr) -> None:
    await bsmr.build(":optional_passing")

    # Optional validations are not run by default.
    await bsmr.build(":optional_failing")

    # Expect a failure when run with --enable-optional-validations.
    await expect_failure(
        bsmr.build(":optional_failing", "--enable-optional-validations", "whistle"),
        stderr_regex="Validation for `.+` failed",
    )
