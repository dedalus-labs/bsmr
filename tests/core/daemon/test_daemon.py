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


import json
import platform
import re
import subprocess
import time
from pathlib import Path

import pytest
from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test, env
from bsmr.tests.e2e_util.helper.utils import daemon_is_alive


@bsmr_test()
@env("BSMR_TESTING_INACTIVITY_TIMEOUT", "true")
async def test_inactivity_timeout(bsmr: Bsmr) -> None:
    #######################################################
    # Recommend running this test in opt mode
    # Otherwise the command that is run here
    # could take longer than 1 second to finish
    # causing this test to be flaky
    #######################################################

    # this will start the daemon
    status = await bsmr.server("--status")
    pid = json.loads(status.stdout)["process_info"]["pid"]
    daemon_dir = await bsmr.get_daemon_dir()

    time.sleep(1)  # 1 sec timeout

    # check it's dead
    for _ in range(20):
        time.sleep(1)
        if not daemon_is_alive(pid):
            result = await bsmr.status()
            assert "no bsmrd running" == result.stderr.splitlines()[-1]

            stderr = (daemon_dir / "bsmrd.stderr").read_text()
            assert "inactivity timeout elapsed" in stderr
            return

    raise AssertionError(f"Server with pid {pid} did not die in 20 seconds")


@bsmr_test()
async def test_server_endpoint_output(bsmr: Bsmr) -> None:
    result = await bsmr.server()
    stdout = result.stdout.strip()
    assert stdout.startswith("bsmrd.endpoint=")
    assert stdout.removeprefix("bsmrd.endpoint=")


@bsmr_test()
async def test_server_status_output(bsmr: Bsmr) -> None:
    result = await bsmr.server("--status")
    status = json.loads(result.stdout)
    pid = status["process_info"]["pid"]
    assert isinstance(pid, int)
    assert pid > 0


@bsmr_test()
async def test_server_status_snapshot_output(bsmr: Bsmr) -> None:
    result = await bsmr.server("--status", "--snapshot")
    status = json.loads(result.stdout)
    snapshot = status["snapshot"]
    assert snapshot is not None
    assert "bsmr_max_rss" in snapshot


@bsmr_test()
async def test_server_snapshot_requires_status(bsmr: Bsmr) -> None:
    await expect_failure(bsmr.server("--snapshot"), stderr_regex="--status")


@bsmr_test()
@pytest.mark.parametrize(
    "corrupt",
    ["not-json", '{"valid-json", "but-not-valid-data"}'],
)
async def test_corrupted_bsmrd_info(bsmr: Bsmr, corrupt: str) -> None:
    await bsmr.targets("//:rule")

    daemon_dir = await bsmr.get_daemon_dir()
    with open(f"{daemon_dir}/bsmrd.info") as f:
        # Check file exists and valid.
        json.load(f)

    # Kill that daemon now to avoid having making a mess and leaving 2 daemons
    # around.
    await bsmr.kill()

    with open(f"{daemon_dir}/bsmrd.info", "w") as f:
        f.write(corrupt)

    await bsmr.targets("//:rule")


@bsmr_test()
async def test_process_title(bsmr: Bsmr) -> None:
    await bsmr.build()  # Start the daemon
    status = await bsmr.status()
    status = json.loads(status.stdout)
    pid = status["process_info"]["pid"]

    if platform.system() == "Darwin":
        out = subprocess.check_output(["ps", "-o", "comm=", str(pid)]).strip()
        assert out.startswith(b"bsmrd[")
    elif platform.system() == "Linux":
        out = subprocess.check_output(["ps", "-o", "cmd=", str(pid)]).strip()
        assert out.startswith(b"bsmrd[")
    elif platform.system() == "Windows":
        # We guarantee no value there.
        pass
    else:
        raise Exception("Unknown platform")


@bsmr_test()
async def test_status_fields(bsmr: Bsmr) -> None:
    await bsmr.build()  # Start the daemon
    status = await bsmr.status()
    status = json.loads(status.stdout)
    assert status["valid_working_directory"]
    assert status["valid_output_mount"]


@bsmr_test()
async def test_status_all(bsmr: Bsmr) -> None:
    # this will start the daemons
    await bsmr.server()

    status = await bsmr.status()
    status = json.loads(status.stdout)
    pid = status["process_info"]["pid"]

    status_all = await bsmr.status("--all")
    status_all = json.loads(status_all.stdout)
    for status in status_all:
        if status["process_info"]["pid"] == pid:
            return
    raise Exception(
        f"bsmrd status for pid {pid} not found in {json.dumps(status_all, indent=2)}"
    )


@bsmr_test()
@env("BSMR_LOG", "bsmr_client_ctx::daemon::client::kill=debug")
async def test_no_daemon_kills_existing_daemon(bsmr: Bsmr) -> None:
    await bsmr.audit("cell")  # Start the daemon
    result = await bsmr.audit("cell", "--no-bsmrd")  # Kill the existing daemon
    assert "Killing daemon with PID" in result.stderr


@bsmr_test()
async def test_output_is_cache_dir(bsmr: Bsmr) -> None:
    await bsmr.targets(":")  # Start a daemon
    root = await bsmr.root()
    assert (
        (Path(root.stdout.strip()) / "bsmr-out" / "v2" / "CACHEDIR.TAG")
        .read_text(encoding="utf-8")
        .startswith("Signature: 8a477f597d28d172789f06886806bc55")
    )


@bsmr_test()
async def test_prev_daemon_dir(bsmr: Bsmr) -> None:
    await bsmr.targets(":")  # Start a daemon
    await bsmr.kill()
    await bsmr.targets(":")  # Start another daemon

    def extract_pid(stderr: str) -> int:
        pid = [re.match(r".* PID: (\d+)", line) for line in stderr.splitlines()]
        pid = list(filter(None, pid))
        assert len(pid) == 1, pid[0]
        return int(pid[0].group(1))

    new_daemon_stderr = await bsmr.daemon_stderr()
    killed_daemon_stderr = await bsmr.prev_daemon_stderr()

    # check logs contain bsmrd pid and don't match
    assert extract_pid(new_daemon_stderr) != extract_pid(killed_daemon_stderr)

    assert "triggered shutdown: `bsmr kill` was invoked" in killed_daemon_stderr
