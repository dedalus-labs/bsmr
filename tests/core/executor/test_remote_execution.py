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

import os
import tempfile

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test, env
from bsmr.tests.e2e_util.helper.utils import filter_events, random_string


@bsmr_test()
@env("BSMR_TEST_FAIL_CONNECT", "true")
async def test_re_connection_failure_no_retry(bsmr: Bsmr) -> None:
    out = await expect_failure(
        bsmr.build(
            "root//:simple",
            "--remote-only",
            "--no-remote-cache",
        ),
    )

    assert "Injected RE Connection error" in out.stderr
    assert "retrying after sleeping" not in out.stderr


RE_USE_CASE_STAGES = ("Queue", "WorkerDownload", "Execute", "WorkerUpload")


async def assert_re_use_case(bsmr: Bsmr, expected_use_case: str) -> None:
    for action in RE_USE_CASE_STAGES:
        use_cases = await filter_events(
            bsmr,
            "Event",
            "data",
            "SpanStart",
            "data",
            "ExecutorStage",
            "stage",
            "Re",
            "stage",
            action,
            "use_case",
        )
        assert use_cases, f"No RE `{action}` stages found"
        assert all(use_case == expected_use_case for use_case in use_cases), use_cases


@bsmr_test()
async def test_re_use_case_override_with_arg(bsmr: Bsmr) -> None:
    # Make sure action is not cached
    with open(bsmr.cwd / "input.txt", "w") as f:
        f.write(random_string())
    await bsmr.build(
        "root//:simple",
        "--remote-only",
        "--no-remote-cache",
    )
    await assert_re_use_case(bsmr, "bsmr-testing")
    # Change the target input
    with open(bsmr.cwd / "input.txt", "w") as f:
        f.write(random_string())
    await bsmr.build(
        "root//:simple",
        "--remote-only",
        "--no-remote-cache",
        "--config",
        "bsmr_re_client.override_use_case=bsmr-user",
    )
    await assert_re_use_case(bsmr, "bsmr-user")


@bsmr_test()
async def test_re_use_case_override_with_config(bsmr: Bsmr) -> None:
    # Make sure action is not cached
    with open(bsmr.cwd / "input.txt", "w") as f:
        f.write(random_string())
    await bsmr.build(
        "root//:simple",
        "--remote-only",
        "--no-remote-cache",
    )
    await assert_re_use_case(bsmr, "bsmr-testing")
    # Change the target input
    with open(bsmr.cwd / "input.txt", "w") as f:
        f.write(random_string())
    with open(bsmr.cwd / ".bsmr.local", "w") as f:
        f.write("[bsmr_re_client]\n")
        f.write("override_use_case = bsmr-user\n")
    await bsmr.build(
        "root//:simple",
        "--remote-only",
        "--no-remote-cache",
    )
    await assert_re_use_case(bsmr, "bsmr-user")


@bsmr_test()
async def test_re_use_case_override_with_external_config(bsmr: Bsmr) -> None:
    # Make sure action is not cached
    with open(bsmr.cwd / "input.txt", "w") as f:
        f.write(random_string())
    await bsmr.build(
        "root//:simple",
        "--remote-only",
        "--no-remote-cache",
    )
    await assert_re_use_case(bsmr, "bsmr-testing")
    # Change the target input
    with open(bsmr.cwd / "input.txt", "w") as f:
        f.write(random_string())
    with tempfile.NamedTemporaryFile("w", delete=False) as f:
        f.write("[bsmr_re_client]\n")
        f.write("override_use_case = bsmr-user\n")
        f.close()
        await bsmr.build(
            "root//:simple",
            "--remote-only",
            "--no-remote-cache",
            "--config-file",
            f.name,
        )
    await assert_re_use_case(bsmr, "bsmr-user")


@bsmr_test()
async def test_re_use_case_override_with_external_config_source(bsmr: Bsmr) -> None:
    with tempfile.NamedTemporaryFile("w", delete=False) as temp:
        env = os.environ.copy()
        env["BSMR_TEST_EXTRA_EXTERNAL_CONFIG"] = temp.name
        # Make sure action is not cached
        with open(bsmr.cwd / "input.txt", "w") as f:
            f.write(random_string())
        await bsmr.build(
            "root//:simple",
            "--remote-only",
            "--no-remote-cache",
            env=env,
        )
        await assert_re_use_case(bsmr, "bsmr-default")
        # Change the target input
        with open(bsmr.cwd / "input.txt", "w") as f:
            f.write(random_string())
        temp.write("[bsmr_re_client]\n")
        temp.write("override_use_case = bsmr-user\n")
        temp.flush()
        await bsmr.build(
            "root//:simple",
            "--remote-only",
            "--no-remote-cache",
            env=env,
        )
        await assert_re_use_case(bsmr, "bsmr-user")
