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
import json
import os

import pytest
from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.api.bsmr_result import BsmrException
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test, env


# Length-prefixed protobuf frame for:
# `SubscriptionRequest { subscribe_to_active_commands: SubscribeToActiveCommands {} }`.
# This test intentionally uses the raw frame to verify stdin requests keep the
# daemon alive without going through the CLI's `--active-commands` helper. The
# wire shape is stable enough for this test: the subscription API is part of
# Bessemer's client/daemon protocol, and the existing field number for
# `subscribe_to_active_commands` must remain backward-compatible.
SUBSCRIBE_TO_ACTIVE_COMMANDS_REQUEST = b"\x02\x22\x00"


@bsmr_test()
async def test_subscribe(bsmr: Bsmr) -> None:
    path = (await bsmr.targets("//:stage1", "--show-output")).stdout.strip().split()[1]

    # Bessemer wants normalized paths here.
    path = path.replace("\\", "/")

    expect = os.environ["BSMR_EXPECT"]
    args = [
        "--bsmr",
        bsmr.path_to_executable,
        path,
    ]

    if bsmr.isolation_prefix is not None:
        args.extend(
            [
                "--isolation-dir",
                bsmr.isolation_prefix,
            ]
        )

    proc = await asyncio.create_subprocess_exec(
        expect,
        *args,
        stdout=asyncio.subprocess.PIPE,
        stderr=asyncio.subprocess.PIPE,
        cwd=bsmr.cwd,
        env=bsmr._env,
    )

    await bsmr.build("//:stage2")

    # We don't expect this to actually take anywhere near 20 seconds, but on CI
    # on a busy host this could take a while.
    (stdout, stderr) = await asyncio.wait_for(proc.communicate(), timeout=20)
    assert proc.returncode == 0
    assert stdout.strip().decode("utf-8") == path


@bsmr_test()
async def test_active_commands(bsmr: Bsmr) -> None:
    async with await bsmr.subscribe("--active-commands") as subscribe:
        msg = await subscribe.read_message()
        commands = msg["response"]["ActiveCommandsSnapshot"]["active_commands"]
        assert len(commands) == 1
        assert "subscribe" in commands[0]["argv"]


@bsmr_test()
async def test_disconnect_eof(bsmr: Bsmr) -> None:
    async with await bsmr.subscribe() as subscribe:
        subscribe.stdin.close()
        msg = await subscribe.read_message()
        assert "EOF" in msg["response"]["Goodbye"]["reason"]


@bsmr_test()
@env("BSMR_TESTING_INACTIVITY_TIMEOUT", "true")
async def test_requests_keep_daemon_alive(bsmr: Bsmr) -> None:
    async with await bsmr.subscribe() as subscribe:
        subscribe.stdin.write(SUBSCRIBE_TO_ACTIVE_COMMANDS_REQUEST)
        await subscribe.stdin.drain()
        await subscribe.read_message()

        pid = json.loads((await bsmr.status()).stdout)["process_info"]["pid"]

        for _ in range(3):
            await asyncio.sleep(0.6)
            subscribe.stdin.write(SUBSCRIBE_TO_ACTIVE_COMMANDS_REQUEST)
            await subscribe.stdin.drain()
            await subscribe.read_message()

        status = json.loads((await bsmr.status()).stdout)
        assert status["process_info"]["pid"] == pid
        assert subscribe._process.returncode is None


@bsmr_test()
async def test_disconnect_error(bsmr: Bsmr) -> None:
    with pytest.raises(BsmrException):
        async with await bsmr.subscribe() as subscribe:
            subscribe.stdin.write(b"x")
            subscribe.stdin.close()
            msg = await subscribe.read_message()
            assert "Error parsing request" in msg["response"]["Goodbye"]["reason"]


@bsmr_test()
async def test_unknown_request_error(bsmr: Bsmr) -> None:
    with pytest.raises(BsmrException):
        async with await bsmr.subscribe() as subscribe:
            subscribe.stdin.write(b"\x00")  # Would decode to a None request
