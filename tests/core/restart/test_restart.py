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
import signal

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test

TEST_DIGEST = "76f7aea8c1fc400287312b9608ceb24848ba02ac:14"


@bsmr_test()
async def test_restart_requires_no_stdout(bsmr: Bsmr) -> None:
    res = await bsmr.targets("//:stage0", env={"FORCE_WANT_RESTART": "true"})
    assert res.stdout.count("//:stage0") == 1


@bsmr_test()
async def test_restart(bsmr: Bsmr) -> None:
    # Normally shows once.
    res = await expect_failure(bsmr.targets("//:invalid"))
    assert res.stderr.count("Unknown target `invalid`") == 1

    # But if we force a restart...
    res = await expect_failure(
        bsmr.targets("//:invalid", env={"FORCE_WANT_RESTART": "true"})
    )
    assert res.stderr.count("Unknown target `invalid`") == 2


@bsmr_test(allow_soft_errors=True)
async def test_restart_materializer_corruption(bsmr: Bsmr) -> None:
    stage1 = "//:stage1"
    res = await bsmr.build(stage1)
    out = res.get_build_report().output_for_target(stage1)

    # Now we remove this file (which comes to us via RE)
    # Only way to get it back is by killing the materializer state.
    os.unlink(out)

    res = await bsmr.build("//:stage2")
    assert "Your command will now restart" in res.stderr


@bsmr_test(allow_soft_errors=True)
async def test_restart_cas_missing(bsmr: Bsmr) -> None:
    # Make sure Bsmr is not running.
    await bsmr.kill()

    # Start a daemon with the `src` file tombstoned. This means we cannot download it from RE.
    # This is just the hash of `src`.
    await bsmr.build(env={"BSMR_TEST_TOMBSTONED_DIGESTS": TEST_DIGEST})

    # Now build //:stage2. Bessemer must try to download the file, fail, then
    # restart the daemon.
    res = await bsmr.build("//:stage2")
    assert "Your command will now restart" in res.stderr

    # TODO: We should also handle the case where the top-level artifact is what
    # fails to download (i.e. build stage1 here instead).


@bsmr_test(
    allow_soft_errors=True,
    skip_for_os=["windows"],
)
async def test_restart_forkserver_crash(bsmr: Bsmr) -> None:
    # Start the daemon
    await bsmr.build()

    # Kill its forkserver.
    forkserver_pid = json.loads((await bsmr.status()).stdout)["forkserver_pid"]
    assert forkserver_pid is not None
    os.kill(forkserver_pid, signal.SIGKILL)

    # Wait for its forkserver to exit.
    for _ in range(10):
        try:
            os.kill(forkserver_pid, 0)
        except OSError:
            break
        else:
            await asyncio.sleep(1)

    # Now build a thing and check we restart
    res = await bsmr.build("//:stage2")
    assert "Your command will now restart" in res.stderr


@bsmr_test()
async def test_restart_disabled(bsmr: Bsmr) -> None:
    # Ensure no daemon
    await bsmr.kill()

    with open(bsmr.cwd / ".bsmr", "a") as f:
        f.write("[bsmr]\nrestarter = false")

    result = await expect_failure(
        bsmr.build(
            "//:stage2",
            env={"BSMR_TEST_TOMBSTONED_DIGESTS": TEST_DIGEST},
        ),
    )
    assert "Your command will now restart" not in result.stderr


@bsmr_test(write_invocation_record=True)
async def test_trace_id(bsmr: Bsmr) -> None:
    trace_id = "aaaaaaaa-bbbb-cccc-dddd-eeeeeeeeeeee"

    # But if we force a restart...
    res = await expect_failure(
        bsmr.targets(
            "//:invalid",
            env={"FORCE_WANT_RESTART": "true", "BSMR_WRAPPER_UUID": trace_id},
        )
    )
    record = res.invocation_record()
    assert record["trace_id"] != trace_id
    assert record["restarted_trace_id"] == trace_id
    assert record["should_restart"] is False
