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

from __future__ import annotations

import os
from pathlib import Path
from tempfile import TemporaryDirectory

import pytest
from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.api.bsmr_result import BsmrResult
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test, env
from bsmr.tests.e2e_util.helper.utils import filter_events


# To not fail listing on Mac or Windows
def test_dummy() -> None:
    pass


def _configure(bsmr: Bsmr, kill_and_retry: bool) -> None:
    with open(bsmr.cwd / ".bsmr.local", "w") as f:
        f.write("[bsmr_resource_control]\n")
        if kill_and_retry:
            f.write("preferred_action_suspend_strategy = kill_and_retry\n")
        else:
            f.write("preferred_action_suspend_strategy = cgroup_freeze\n")


def _use_some_memory_args(bsmr: Bsmr, temp: TemporaryDirectory[str]) -> list[str]:
    return [
        "--show-full-simple-output",
        "-c",
        f"use_some_memory.path={os.environ['USE_SOME_MEMORY_BIN']}",
        "-c",
        f"start_marker_files.path={temp.name}",
        "--no-remote-cache",
        "--local-only",
    ]


async def _check_suspends(  # noqa C901
    bsmr: Bsmr,
    kill_and_retry: bool,
    temp: TemporaryDirectory[str],
    res: BsmrResult,
) -> int:
    # First check the reported suspensions
    actions = await filter_events(
        bsmr,
        "Event",
        "data",
        "SpanEnd",
        "data",
        "ActionExecution",
    )
    reported_suspends = {}
    num_suspended_actions = 0
    for action in actions:
        if action["name"]["category"] != "memory_allocating_actions":
            continue
        ident = action["name"]["identifier"]
        command_meta = action["commands"][-1]["details"]["metadata"]
        if command_meta["suspend_duration"] is not None:
            num_suspended_actions += 1
            if kill_and_retry:
                count = command_meta["suspend_count"]
                assert count is not None
                reported_suspends[ident] = count
            else:
                # Json representation of a duration is number of us
                duration = command_meta["suspend_duration"] / 1000
                assert duration is not None
                reported_suspends[ident] = duration
        else:
            reported_suspends[ident] = None

    if kill_and_retry:
        total_detected_kills = 0
        expected_kills = 0
        for ident, count in reported_suspends.items():
            detected_starts = len((Path(temp.name) / ident).read_text().splitlines())
            if count is None:
                assert detected_starts == 1
            else:
                # We can't quite assert that the kills were observed by the action because sometimes
                # we might kill the thing before it gets there. So add them up and assert that we're
                # close enough
                total_detected_kills += detected_starts - 1
                expected_kills += count
        assert total_detected_kills >= expected_kills - 2
    else:
        paths = Path(res.stdout.strip()).read_text().splitlines()
        paths = [bsmr.cwd / p for p in paths]
        assert len(paths) == len(reported_suspends)

        # Then compare them to the detected suspensions
        for p in paths:
            contents = p.read_text()
            ident = p.name
            total_duration: int | None = None
            for line in contents.splitlines():
                if line.startswith("freeze_detected_ms "):
                    this_duration = int(line[len("freeze_detected_ms ") :])
                    total_duration = (total_duration or 0) + this_duration
            if total_duration is not None:
                assert abs(reported_suspends[ident] - total_duration) < 500
            else:
                assert (reported_suspends[ident] or 0) < 500

    return num_suspended_actions


@bsmr_test(skip_for_os=["darwin", "windows"], disable_daemon_cgroup=False)
@env("BSMR_HARD_ERROR", "panic")
@pytest.mark.parametrize("kill_and_retry", [True, False])
async def test_action_suspend(
    bsmr: Bsmr,
    kill_and_retry: bool,
) -> None:
    temp = TemporaryDirectory()
    _configure(bsmr, kill_and_retry)
    res = await bsmr.build_without_report(
        ":sleep_10",
        *_use_some_memory_args(bsmr, temp),
    )

    await _check_suspends(bsmr, kill_and_retry, temp, res)

    pressure_starts = await filter_events(
        bsmr,
        "Event",
        "data",
        "SpanStart",
        "data",
        "MemoryPressure",
    )
    pressure_ends = await filter_events(
        bsmr,
        "Event",
        "data",
        "SpanEnd",
        "data",
        "MemoryPressure",
    )
    assert len(pressure_starts) == 1
    assert len(pressure_ends) == 1


@bsmr_test(skip_for_os=["darwin", "windows"], disable_daemon_cgroup=False)
@env("BSMR_HARD_ERROR", "panic")
@pytest.mark.parametrize("kill_and_retry", [True, False])
async def test_action_suspend_stress_test(
    bsmr: Bsmr,
    kill_and_retry: bool,
) -> None:
    temp = TemporaryDirectory()
    _configure(bsmr, kill_and_retry)
    await bsmr.build(
        ":very_fast_100",
        *_use_some_memory_args(bsmr, temp),
    )


@bsmr_test(skip_for_os=["darwin", "windows"], disable_daemon_cgroup=False)
@pytest.mark.skip(
    reason="Action suspension not triggering in CI, same as test_action_suspend"
)
@pytest.mark.parametrize("kill_and_retry", [True, False])
async def test_suspend_one_of_two(
    bsmr: Bsmr,
    kill_and_retry: bool,
) -> None:
    temp = TemporaryDirectory()
    _configure(bsmr, kill_and_retry)

    res = await bsmr.build_without_report(
        ":two_mutually_incompatible",
        *_use_some_memory_args(bsmr, temp),
    )

    num_suspends = await _check_suspends(bsmr, kill_and_retry, temp, res)
    assert num_suspends == 1
