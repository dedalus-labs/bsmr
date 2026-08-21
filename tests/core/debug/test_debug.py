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


import os.path
from pathlib import Path

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


@bsmr_test()
async def test_debug_crash(bsmr: Bsmr) -> None:
    # If the first operation immediately does a panic then we fail to connect.
    # While that's not great, having some panics is better than none, so test once after we spawn.
    await bsmr.build()
    result = await expect_failure(bsmr.debug("crash", "panic"))
    assert "explicitly requested panic" in result.stderr
    # Our crash output should include a stack trace.
    assert "stack backtrace:" in result.stderr


@bsmr_test()
async def test_debug_exe(bsmr: Bsmr) -> None:
    result = await bsmr.debug("exe")
    path = result.stdout.strip()
    assert os.path.exists(path)


@bsmr_test()
async def test_debug_allocative(bsmr: Bsmr, tmp_path: Path) -> None:
    # Start the server.
    await bsmr.uquery("root//:")

    file_path = tmp_path / "profile"

    output = await bsmr.debug("allocative", "--output", str(file_path))
    assert os.path.exists(f"{file_path}/flame.src")
    assert os.path.exists(f"{file_path}/flame.svg")
    assert "Allocative profile written to" in output.stderr

    await bsmr.debug("allocative")
    assert os.path.exists(bsmr.cwd / "allocative-out" / "flame.src")
    assert os.path.exists(bsmr.cwd / "allocative-out" / "flame.svg")


@bsmr_test()
async def test_debug_filestatus(bsmr: Bsmr) -> None:
    # Start the server.
    await bsmr.uquery("root//:")
    # FIXME(JakobDegen): `.` is an error
    output = await bsmr.debug("file-status", "TARGETS.fixture")
    assert "No mismatches detected" in output.stderr


@bsmr_test()
async def test_debug_flush_pgo_profile(bsmr: Bsmr) -> None:
    await bsmr.build()
    result = await bsmr.debug("flush-pgo-profile")
    assert "was not flushed" in result.stderr


@bsmr_test(skip_for_os=["windows", "darwin"])
async def test_thread_dump(bsmr: Bsmr) -> None:
    # Make sure we don't start a daemon if there isn't one
    await expect_failure(
        bsmr.debug("thread-dump"),
        stderr_regex="No running bsmr daemon",
    )
    # Start the daemon
    await bsmr.uquery("root//:")
    output = await bsmr.debug("thread-dump")
    assert "frame #0" in output.stdout
