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


import typing
from pathlib import Path

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test
from bsmr.tests.e2e_util.helper.utils import filter_events


async def check_targets(
    bsmr: Bsmr,
    expected_target_names: typing.List[str],
    expected_error_messages: typing.List[str],
) -> None:
    build_response = await filter_events(
        bsmr,
        "Result",
        "result",
        "build_response",
    )
    build_response = build_response[0]
    build_targets = build_response["build_targets"]
    assert len(build_targets) == len(expected_target_names)
    for actual, expected in zip(build_targets, expected_target_names):
        if expected is not None:
            assert actual["target"] == expected
    error_messages = build_response["errors"]
    assert len(error_messages) == len(expected_error_messages)
    for actual_msg, expected in zip(error_messages, expected_error_messages):
        if expected is not None:
            assert expected in actual_msg["message"]


@bsmr_test()
async def test_build_one_fails(bsmr: Bsmr, tmp_path: Path) -> None:
    report = tmp_path / "build-report.json"
    await expect_failure(
        bsmr.build(
            "--build-report",
            str(report),
            "//:fail",
            "//:a_one",
        ),
        stderr_regex="Failed to build 'root//:fail",
    )
    await check_targets(
        bsmr,
        ["root//:a_one", "root//:fail"],
        ["Failed to build 'root//:fail (<unspecified>)'"],
    )
