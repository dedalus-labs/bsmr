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
import typing

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


@bsmr_test(skip_for_os=["darwin", "windows"])
async def test_default_output(bsmr: Bsmr) -> None:
    status = await start_daemon_and_get_status(bsmr)
    assert "tokio_runtime_metrics" not in status


@bsmr_test(skip_for_os=["darwin", "windows"])
async def test_includes_tokio(bsmr: Bsmr) -> None:
    status = await start_daemon_and_get_status(bsmr, include_tokio_runtime_metrics=True)
    assert "tokio_runtime_metrics" in status
    metrics = status["tokio_runtime_metrics"]
    assert "global_queue_depth" in metrics
    assert "num_alive_tasks" in metrics
    assert "num_workers" in metrics


@bsmr_test(skip_for_os=["darwin", "windows"])
async def test_notices_tokio_worker_override(bsmr: Bsmr) -> None:
    append_tokio_workers_config(bsmr)
    status = await start_daemon_and_get_status(bsmr, include_tokio_runtime_metrics=True)
    assert "tokio_runtime_metrics" in status
    metrics = status["tokio_runtime_metrics"]
    assert "num_workers" in metrics
    assert metrics["num_workers"] == 42  # from modification below


@bsmr_test()
async def test_snapshot_events_carry_tokio_runtime_stats(bsmr: Bsmr) -> None:
    # Snapshots should always include tokio_runtime_stats — there's no toggle.
    await bsmr.targets(":")
    snapshots = await _snapshot_events(bsmr)
    assert len(snapshots) > 0, "expected at least one snapshot event in the log"
    populated = [s for s in snapshots if s.get("tokio_runtime_stats")]
    assert len(populated) > 0, (
        "expected at least one snapshot to carry tokio_runtime_stats"
    )
    stats = populated[-1]["tokio_runtime_stats"]
    assert "workers" in stats
    assert len(stats["workers"]) > 0
    # At least one worker should have done some work, exposing cumulative
    # counters. (Default-zero u64 fields are omitted by proto3 JSON, so we
    # look for a worker that has any of them present.)
    counter_fields = ("park_unpark_count", "poll_count", "total_busy_duration_us")
    assert any(any(f in w for f in counter_fields) for w in stats["workers"]), (
        f"expected some worker to expose any of {counter_fields}, got {stats['workers']}"
    )


async def _snapshot_events(bsmr: Bsmr) -> list[dict[str, typing.Any]]:
    """Returns the Snapshot payloads from the most recent invocation's log."""
    out = await bsmr.log("show")
    snapshots: list[dict[str, typing.Any]] = []
    for line in out.stdout.splitlines():
        if not line:
            continue
        event = json.loads(line)
        instant = (
            event.get("Event", {}).get("data", {}).get("Instant", {}).get("data", {})
        )
        if "Snapshot" in instant:
            snapshots.append(instant["Snapshot"])
    return snapshots


def append_tokio_workers_config(bsmr: Bsmr) -> None:
    with open(bsmr.cwd / ".bsmr", "a") as bsmrconfig:
        bsmrconfig.write("[build]\n")
        bsmrconfig.write("num_tokio_workers = 42")


async def start_daemon_and_get_status(
    bsmr: Bsmr, include_tokio_runtime_metrics: bool = False
) -> dict[str, typing.Any]:
    # Start the daemon
    await bsmr.targets(":")

    status_result = await (
        bsmr.status("--include-tokio-runtime-metrics")
        if include_tokio_runtime_metrics
        else bsmr.status()
    )
    status_data = json.loads(status_result.stdout)
    return status_data
