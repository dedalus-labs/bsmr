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


import asyncio
import os
import signal
from collections.abc import Callable
from pathlib import Path

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.api.bsmr_result import BsmrException, BsmrResult, ExitCodeV2
from bsmr.tests.e2e_util.api.process import Process
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test
from bsmr.tests.e2e_util.helper.utils import read_invocation_record


async def _test_cancellation_helper(
    bsmr: Bsmr,
    tmp_path: Path,
    runner: Callable[[Bsmr, list[str]], Process[BsmrResult, BsmrException]],
) -> None:
    """
    This test starts a test that writes its PID to a file then runs for 60
    seconds. We test cancellation by sending a CTRL+C as soon as a test
    starts. We then check that the process exited, and that nothing else
    started (or if anything did, that they stopped).
    """
    pid_path = tmp_path / "pids"
    pid_path.mkdir()
    record_path = tmp_path / "record.json"
    opts = [
        "-c",
        f"test.pids={pid_path}",
        "-c",
        "test.duration=60",
        "--unstable-write-invocation-record",
        str(record_path),
    ]
    await bsmr.audit("providers", ":slow", *opts)
    command = runner(bsmr, [*opts, "--local-only"])

    command = await command.start()

    for _i in range(30):
        await asyncio.sleep(1)
        pids = os.listdir(pid_path)
        if pids:
            break
    else:
        raise Exception("Commands never started")

    command.send_signal(signal.SIGINT)
    await command.communicate()  # Wait for the command to exit

    # Give stuff time to settle, PIDS don't necessarily disappear
    # instantly. Also, verify that we are not starting more tests.
    await asyncio.sleep(5)

    # At this point, nothing should be alive.
    pids = os.listdir(pid_path)
    for pid in pids:
        try:
            os.kill(int(pid), 0)
        except OSError:
            pass
        else:
            raise Exception(f"PID existed: {pid}")

    record = read_invocation_record(record_path)
    assert record["exit_code"] == ExitCodeV2.SIGNAL_INTERRUPT.value
    assert record["exit_result_name"] == "SIGNAL_INTERRUPT"


@bsmr_test()
async def test_cancellation(bsmr: Bsmr, tmp_path: Path) -> None:
    await _test_cancellation_helper(
        bsmr, tmp_path, lambda bsmr, opts: bsmr.build(*opts, ":slow")
    )


@bsmr_test()
async def test_cancellation_bxl(bsmr: Bsmr, tmp_path: Path) -> None:
    await _test_cancellation_helper(
        bsmr, tmp_path, lambda bsmr, opts: bsmr.bxl(*opts, "//build.bxl:build")
    )
