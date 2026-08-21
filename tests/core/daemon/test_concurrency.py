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
from typing import Optional

import pytest
from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.api.bsmr_result import BsmrException, BuildResult
from bsmr.tests.e2e_util.api.process import Process
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


@bsmr_test()
async def test_exit_when_different_state(bsmr: Bsmr) -> None:
    a = bsmr.build(
        "-c",
        "foo.bar=1",
        "--exit-when=differentstate",
        ":long_running_target",
        "--local-only",
        "--no-remote-cache",
    )

    b = bsmr.build(
        "-c",
        "foo.bar=2",
        "--exit-when=differentstate",
        ":long_running_target",
        "--local-only",
        "--no-remote-cache",
    )

    # create a coroutine that can return a result
    async def process(
        p: Process[BuildResult, BsmrException],
    ) -> tuple[Optional[int], str]:
        result = await expect_failure(p)
        return (result.process.returncode, result.stderr)

    done, pending = await asyncio.wait(
        [asyncio.create_task(process(a)), asyncio.create_task(process(b))],
        timeout=10,
        return_when=asyncio.FIRST_COMPLETED,
    )

    assert len(done) == 1
    assert len(pending) == 1

    # these are sets, so can't index them.
    for task in done:
        exit_code, stderr = task.result()
        assert "daemon is busy" in stderr
        assert exit_code == 4


@bsmr_test()
@pytest.mark.parametrize("same_state", [True, False])
async def test_exit_when_preemptible_always(bsmr: Bsmr, same_state: bool) -> None:
    a = bsmr.build(
        "-c",
        "foo.bar=1",
        "--preemptible=always",
        ":long_running_target",
        "--local-only",
        "--no-remote-cache",
    )

    b = bsmr.build(
        "-c",
        # We expect to ALWAYS preempt commands, to prevent blocking new callees
        "foo.bar=1" if same_state else "foo.bar=2",
        "--preemptible=always",
        ":long_running_target",
        "--local-only",
        "--no-remote-cache",
    )

    # create a coroutine that can return a result
    async def process(
        p: Process[BuildResult, BsmrException],
    ) -> tuple[Optional[int], str]:
        result = await expect_failure(p)
        return (result.process.returncode, result.stderr)

    done, pending = await asyncio.wait(
        [asyncio.create_task(process(a)), asyncio.create_task(process(b))],
        timeout=10,
        return_when=asyncio.FIRST_COMPLETED,
    )

    assert len(done) == 1
    assert len(pending) == 1

    # these are sets, so can't index them.
    for task in done:
        exit_code, stderr = task.result()
        assert "daemon preempted" in stderr
        assert exit_code == 5


@bsmr_test(write_invocation_record=True)
async def test_preemptible_logged(bsmr: Bsmr) -> None:
    res = await bsmr.targets(
        "--preemptible=always",
        ":",
    )
    record = res.invocation_record()
    assert record["preemptible"] == "ALWAYS"


@bsmr_test()
@pytest.mark.parametrize("same_state", [True, False])
async def test_exit_when_preemptible_on_different_state(
    bsmr: Bsmr, same_state: bool
) -> None:
    a = bsmr.build(
        "-c",
        "foo.bar=1",
        "--preemptible=ondifferentstate",
        ":long_running_target",
        "--local-only",
        "--no-remote-cache",
    )

    b = bsmr.build(
        "-c",
        # We expect to ALWAYS preempt commands, to prevent blocking new callees
        "foo.bar=1" if same_state else "foo.bar=2",
        "--preemptible=ondifferentstate",
        ":long_running_target",
        "--local-only",
        "--no-remote-cache",
    )

    # create a coroutine that can return a result
    async def process(
        p: Process[BuildResult, BsmrException],
    ) -> tuple[Optional[int], str]:
        result = await expect_failure(p)
        return (result.process.returncode, result.stderr)

    done, pending = await asyncio.wait(
        [asyncio.create_task(process(a)), asyncio.create_task(process(b))],
        timeout=10,
        return_when=asyncio.FIRST_COMPLETED,
    )

    if same_state:
        # No preempt when state is the same
        assert len(done) == 0
        assert len(pending) == 2
    else:
        assert len(done) == 1
        assert len(pending) == 1

    # These are sets, so can't index them. Expect all done tasks to be "done" because they're preempted
    for task in done:
        exit_code, stderr = task.result()
        assert "daemon preempted" in stderr
        assert exit_code == 5


@bsmr_test()
@pytest.mark.parametrize("same_state", [True, False])
async def test_exit_when_not_idle_does_not_start_when_daemon_busy(
    bsmr: Bsmr, same_state: bool
) -> None:
    """Test that an incoming command with --exit-when=notidle does not start if there's another command running."""

    # create a coroutine that can return a result
    async def process(
        p: Process[BuildResult, BsmrException],
    ) -> tuple[Optional[int], str]:
        result = await expect_failure(p)
        return (result.process.returncode, result.stderr)

    a = bsmr.build(
        "-c",
        "foo.bar=1",
        ":long_running_target",
        "--local-only",
        "--no-remote-cache",
    )
    # Start the first command
    task_a = asyncio.create_task(process(a))

    # Wait a short time to ensure the first command has started
    await asyncio.sleep(0.5)

    b = bsmr.build(
        "-c",
        "foo.bar=1" if same_state else "foo.bar=2",
        "--exit-when=notidle",
        ":long_running_target",
        "--local-only",
        "--no-remote-cache",
    )
    task_b = asyncio.create_task(process(b))

    # Wait for one of the tasks to complete
    done, pending = await asyncio.wait(
        [task_a, task_b],
        timeout=10,
        return_when=asyncio.FIRST_COMPLETED,
    )

    assert len(done) == 1
    assert len(pending) == 1

    # These are sets, so can't index them. Expect all done tasks to be "done" because they're preempted
    for task in done:
        exit_code, stderr = task.result()
        assert "daemon is busy" in stderr
        assert exit_code == 4


@bsmr_test()
@pytest.mark.parametrize("same_state", [True, False])
async def test_exit_when_not_idle_does_not_gets_preempted(
    bsmr: Bsmr, same_state: bool
) -> None:
    """Test that a running command with --exit-when=notidle continues running even with an incoming command."""

    # create a coroutine that can return a result
    async def process(
        p: Process[BuildResult, BsmrException],
    ) -> tuple[Optional[int], str]:
        result = await expect_failure(p)
        return (result.process.returncode, result.stderr)

    a = bsmr.build(
        "-c",
        "foo.bar=1",
        "--exit-when=notidle",
        ":long_running_target",
        "--local-only",
        "--no-remote-cache",
    )
    # Start the first command
    task_a = asyncio.create_task(process(a))

    # Wait a short time to ensure the first command has started
    await asyncio.sleep(0.5)

    # Start another command without the flag (default is --preemptible=never)
    b = bsmr.build(
        "-c",
        "foo.bar=1" if same_state else "foo.bar=2",
        ":long_running_target",
        "--local-only",
        "--no-remote-cache",
    )
    task_b = asyncio.create_task(process(b))

    # Wait for one of the tasks to complete
    done, pending = await asyncio.wait(
        [task_a, task_b],
        timeout=10,
        return_when=asyncio.FIRST_COMPLETED,
    )

    assert len(done) == 0
    assert len(pending) == 2


@bsmr_test()
@pytest.mark.parametrize("same_state", [True, False])
async def test_preemptible_exit_when_not_idle_gets_preempted(
    bsmr: Bsmr, same_state: bool
) -> None:
    """Test that a running command with --exit-when=notidle and --preemptible gets preempted with an incoming command."""

    # create a coroutine that can return a result
    async def process(
        p: Process[BuildResult, BsmrException],
    ) -> tuple[Optional[int], str]:
        result = await expect_failure(p)
        return (result.process.returncode, result.stderr)

    a = bsmr.build(
        "-c",
        "foo.bar=1",
        "--exit-when=notidle",
        "--preemptible=always",
        ":long_running_target",
        "--local-only",
        "--no-remote-cache",
    )
    # Start the first command
    task_a = asyncio.create_task(process(a))

    # Wait a short time to ensure the first command has started
    await asyncio.sleep(0.5)

    # Start another command without the flag (default is --preemptible=never)
    b = bsmr.build(
        "-c",
        "foo.bar=1" if same_state else "foo.bar=2",
        ":long_running_target",
        "--local-only",
        "--no-remote-cache",
    )
    task_b = asyncio.create_task(process(b))

    # Wait for one of the tasks to complete
    done, pending = await asyncio.wait(
        [task_a, task_b],
        timeout=10,
        return_when=asyncio.FIRST_COMPLETED,
    )

    assert len(done) == 1
    assert len(pending) == 1

    # These are sets, so can't index them. Expect all done tasks to be "done" because they're preempted
    for task in done:
        exit_code, stderr = task.result()
        assert "daemon preempted" in stderr
        assert exit_code == 5


@bsmr_test()
@pytest.mark.parametrize("same_state", [True, False])
async def test_multiple_exit_when_not_idle_commands(
    bsmr: Bsmr, same_state: bool
) -> None:
    """
    Test that a running command with --exit-when=notidle does NOT get preempted by an incoming command that
    also has --exit-when=notidle set.
    """

    # create a coroutine that can return a result
    async def process(
        p: Process[BuildResult, BsmrException],
    ) -> tuple[Optional[int], str]:
        result = await expect_failure(p)
        return (result.process.returncode, result.stderr)

    a = bsmr.build(
        "-c",
        "foo.bar=1",
        "--exit-when=notidle",
        ":long_running_target",
        "--local-only",
        "--no-remote-cache",
    )
    # Start the first command
    task_a = asyncio.create_task(process(a))

    # Wait a short time to ensure the first command has started
    await asyncio.sleep(0.5)

    b = bsmr.build(
        "-c",
        "foo.bar=1" if same_state else "foo.bar=2",
        "--exit-when=notidle",
        ":long_running_target",
        "--local-only",
        "--no-remote-cache",
    )
    task_b = asyncio.create_task(process(b))

    done, pending = await asyncio.wait(
        [task_a, task_b],
        timeout=10,
        return_when=asyncio.FIRST_COMPLETED,
    )

    assert len(done) == 1
    assert len(pending) == 1

    # these are sets, so can't index them.
    for task in done:
        exit_code, stderr = task.result()
        assert "daemon is busy" in stderr
        assert exit_code == 4


@bsmr_test()
@pytest.mark.parametrize("same_state", [True, False])
async def test_exit_when_not_idle_after_command_exits(
    bsmr: Bsmr, same_state: bool
) -> None:
    """ """

    # create a coroutine that can return a result
    async def process(
        p: Process[BuildResult, BsmrException],
    ) -> tuple[Optional[int], str]:
        result = await p
        return (result.process.returncode, result.stderr)

    a = bsmr.build(
        "-c",
        "foo.bar=1",
        ":short_running_target",
        "--local-only",
        "--no-remote-cache",
    )
    # Start the first command
    task_a = asyncio.create_task(process(a))

    # Wait a short time to ensure the first command has finished
    await asyncio.sleep(2)

    b = bsmr.build(
        "-c",
        "foo.bar=1" if same_state else "foo.bar=2",
        "--exit-when=notidle",
        ":short_running_target",
        "--local-only",
        "--no-remote-cache",
    )
    task_b = asyncio.create_task(process(b))

    done, pending = await asyncio.wait(
        [task_a, task_b],
        timeout=10,
        return_when=asyncio.ALL_COMPLETED,
    )

    assert len(done) == 2
    assert len(pending) == 0

    # these are sets, so can't index them.
    for task in done:
        exit_code, stderr = task.result()
        assert "daemon is busy" not in stderr
        assert exit_code == 0
