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


import typing
from pathlib import Path

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


def nested_bsmr_args(bsmr: Bsmr) -> typing.List[str]:
    return [
        "-c",
        f"nested.build_path={bsmr.path_to_executable}",
    ]


@bsmr_test(allow_soft_errors=True)
async def test_same_state(bsmr: Bsmr) -> None:
    await bsmr.build(
        "root//:nested_normal", *nested_bsmr_args(bsmr), env={"SANDCASTLE_ID": ""}
    )


@bsmr_test(allow_soft_errors=True)
async def test_different_state_error(bsmr: Bsmr, tmp_path: Path) -> None:
    # FIXME(JakobDegen): Nested invocations seem to have buggy behavior around writing the event
    # logs, so `log show` and friends don't work without this
    log = tmp_path / "logfile.json-lines"
    await expect_failure(
        bsmr.build(
            "-c",
            "some.config=Val",
            "root//:nested_normal",
            "--event-log",
            str(log),
            *nested_bsmr_args(bsmr),
            env={"SANDCASTLE_ID": ""},
        ),
        stderr_regex="Failed to build 'root//:nested_normal",
    )
    res = await bsmr.log("what-ran", "--failed", "--show-std-err", str(log))
    assert "Recursive invocation of Bsmr, with a different state" in res.stdout


@bsmr_test(allow_soft_errors=True)
async def test_different_user_version_and_state(bsmr: Bsmr, tmp_path: Path) -> None:
    log = tmp_path / "logfile.json-lines"
    await expect_failure(
        bsmr.build(
            "-c",
            "some.config=Val",
            "root//:nested_normal",
            "--event-log",
            str(log),
            *nested_bsmr_args(bsmr),
            # Set a `SANDCASTLE_ID`; this affects the daemon constraints
            env={"SANDCASTLE_ID": "12345"},
        ),
        stderr_regex="Failed to build 'root//:nested_normal",
    )
    res = await bsmr.log("what-ran", "--failed", "--show-std-err", str(log))
    assert "Recursive invocation of Bsmr, with a different state" in res.stdout


@bsmr_test(allow_soft_errors=True)
async def test_trace_io_mismatch(bsmr: Bsmr, tmp_path: Path) -> None:
    log = tmp_path / "logfile.json-lines"
    await expect_failure(
        bsmr.build(
            "root//:nested_trace",
            "--event-log",
            str(log),
            *nested_bsmr_args(bsmr),
        ),
        stderr_regex="Failed to build 'root//:nested_trace",
    )
    res = await bsmr.log("what-ran", "--failed", "--show-std-err", str(log))
    assert (
        "daemon constraint mismatch during nested invocation: Trace IO mismatch"
        in res.stdout
    )
