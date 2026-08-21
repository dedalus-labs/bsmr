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

import json
import os
from pathlib import Path

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test
from bsmr.tests.e2e_util.helper.utils import filter_events


# To not fail listing on Mac or Windows
def test_dummy() -> None:
    pass


def _use_some_memory_args(bsmr: Bsmr) -> list[str]:
    return [
        "-c",
        f"use_some_memory.path={os.environ['USE_SOME_MEMORY_BIN']}",
    ]


@bsmr_test(skip_for_os=["darwin", "windows"], disable_daemon_cgroup=False)
async def test_memory_pressure_telemetry(
    bsmr: Bsmr,
) -> None:
    with open(bsmr.cwd / ".bsmr.local", "w") as f:
        f.write("[bsmr_resource_control]\n")
        f.write("memory_high_per_action = 1048576\n")  # 1 MiB

    await bsmr.build(
        ":allocate_10_10M",
        "--no-remote-cache",
        "-c",
        "build.use_limited_hybrid=False",
        "-c",
        "build.execution_platforms=//:platforms",
    )

    resource_control_events = await filter_events(
        bsmr, "Event", "data", "Instant", "data", "ResourceControlEvent"
    )

    # We can't reliably predict how many events will be fired and how high the pressure % will reach,
    # but it should be more than 0 with how low we set the memory_high as and obviously % should be <= 100
    assert len(resource_control_events) > 0
    last_event = resource_control_events[-1]
    pressure = last_event["allprocs_memory_pressure"]
    assert pressure <= 100, (
        f"Expected % memory_pressure to be at most 100, got {pressure}"
    )


@bsmr_test(skip_for_os=["darwin", "windows"], disable_daemon_cgroup=False)
async def test_resource_control_events_created(
    bsmr: Bsmr,
) -> None:
    with open(bsmr.cwd / ".bsmr.local", "w") as f:
        f.write("[bsmr_resource_control]\n")
        f.write("status = required\n")
        f.write("enable_action_cgroup_pool_v2 = true\n")
        f.write(f"memory_high_actions = {200 * 1024 * 1024}\n")  # 200 MiB
        f.write("enable_suspension = true\n")

    await bsmr.build(
        "prelude//:freeze_unfreeze_target",
        "--no-remote-cache",
        "-c",
        "build.use_limited_hybrid=False",
        "-c",
        "build.execution_platforms=//:platforms",
        "--local-only",
        *_use_some_memory_args(bsmr),
    )

    event = await filter_events(
        bsmr,
        "Event",
        "data",
        "Instant",
        "data",
        "ResourceControlEvent",
    )

    # 10 means scheduled event
    assert len(list(filter(lambda e: e["kind"] == 10, event))) > 0
    # assert len(list(filter(lambda e: e["kind"] != 10, event))) > 0


# get the daemon cgroup path
def get_daemon_cgroup_path(pid: int) -> Path:
    cgroup_path = f"/proc/{pid}/cgroup"

    with open(cgroup_path, "r") as f:
        for line in f:
            # cgroup v2 format: 0::/path/to/cgroup
            if line.startswith("0::"):
                cgroup_relative_path = line.strip().split("::", 1)[1]
                return Path(f"/sys/fs/cgroup{cgroup_relative_path}")

    raise Exception(f"Could not find cgroup v2 entry for PID {pid}")


@bsmr_test(skip_for_os=["darwin", "windows"], disable_daemon_cgroup=False)
async def test_daemon_id_in_cgroup_path(bsmr: Bsmr) -> None:
    await bsmr.server()

    result = await bsmr.status()
    status = json.loads(result.stdout)
    daemon_id = status["daemon_constraints"]["daemon_id"]

    daemon_cgroup_path = get_daemon_cgroup_path(status["process_info"]["pid"])

    assert daemon_id.replace("-", "_") in str(daemon_cgroup_path)
