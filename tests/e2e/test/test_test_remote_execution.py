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
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


@bsmr_test(inplace=True)
async def test_re_resource_exhausted_reported_as_infra_failure(bsmr: Bsmr) -> None:
    result = await bsmr.test(
        "--remote-only",
        "--no-remote-cache",
        "root//tests/targets/rules/sh_test:test_remote_explicit",
        "--",
        "--experiment",
        "classify_re_error_as_infra",
        env={
            "BSMR_TEST_FAIL_RE_RESOURCE_EXHAUSTED": "true",
        },
    )
    assert "FATAL" not in result.stderr
    assert "Infra Failure 1" in result.stderr
    assert "resource exhausted" in result.stderr.lower()


@bsmr_test(inplace=True)
async def test_cancel_test_if_re_queue_longer_than_threshold(bsmr: Bsmr) -> None:
    args = [
        "-c",
        "build.remote_execution_cancel_on_estimated_queue_time_exceeds_s=10",
        "--no-remote-cache",
        "--remote-only",
    ]
    result = await bsmr.test(
        *args,
        "root//tests/targets/rules/sh_test:test_remote_explicit_stays_in_queue",
        env={"BSMR_TEST_RE_QUEUE_ESTIMATE_S": "100"},
    )
    assert (
        "Omitted: root//tests/targets/rules/sh_test:test_remote_explicit_stays_in_queue - unmanaged"
        in result.stderr
    )
    assert (
        "The test execution stayed in RE queue for more than threshold time."
        in result.stderr
    )
