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


OUTPUT_LIMIT_EXCEEDED = "(output limit exceeded)"


@bsmr_test()
async def test_noop(bsmr: Bsmr) -> None:
    pass


@bsmr_test(skip_for_os=["darwin", "windows"])
async def test_output_limit_truncates_action_stderr(bsmr: Bsmr) -> None:
    """With a small output limit, action stderr should be truncated."""
    bsmr.set_env("BSMR_CONSOLE_OUTPUT_LIMIT", "10")
    res = await bsmr.build("--console=simplenotty", "-v5", "//:noisy1")
    # The exceeded message should appear exactly once.
    assert res.stderr.count(OUTPUT_LIMIT_EXCEEDED) == 1, res.stderr
    # With a 10-byte limit, most of the output should be suppressed.
    assert res.stderr.count("stderr line") <= 1, res.stderr


@bsmr_test(skip_for_os=["darwin", "windows"])
async def test_output_limit_not_set_prints_all(bsmr: Bsmr) -> None:
    """Without the env var, all output should be printed."""
    res = await bsmr.build("--console=simplenotty", "-v5", "//:noisy1")
    assert OUTPUT_LIMIT_EXCEEDED not in res.stderr, res.stderr
    assert res.stderr.count("stderr line") == 10, res.stderr


@bsmr_test(skip_for_os=["darwin", "windows"])
async def test_output_limit_global_across_actions(bsmr: Bsmr) -> None:
    """The limit is global: building two noisy targets should still only
    print the exceeded message once."""
    bsmr.set_env("BSMR_CONSOLE_OUTPUT_LIMIT", "10")
    res = await bsmr.build("--console=simplenotty", "-v5", "//:noisy1", "//:noisy2")
    assert res.stderr.count(OUTPUT_LIMIT_EXCEEDED) == 1, res.stderr


@bsmr_test(skip_for_os=["darwin", "windows"])
async def test_output_limit_truncates_test_output(bsmr: Bsmr) -> None:
    """With a small output limit, test result output should be truncated."""
    bsmr.set_env("BSMR_CONSOLE_OUTPUT_LIMIT", "10")
    # The test rule exits with 1 so that test result details (stderr) are
    # always printed, letting us verify the output limit applies to them.
    res = await expect_failure(
        bsmr.test("--console=simplenotty", "//:noisy_test"),
    )
    assert res.stderr.count(OUTPUT_LIMIT_EXCEEDED) == 1, res.stderr
    assert res.stderr.count("test output line") <= 1, res.stderr
