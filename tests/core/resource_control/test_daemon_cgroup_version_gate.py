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


@bsmr_test(skip_for_os=["darwin", "windows"], disable_daemon_cgroup=False)
async def test_version_gate_enables_cgroup(bsmr: Bsmr) -> None:
    """When status_if_min_daemon_cgroup_version is set to a version <= the
    binary's DAEMON_CGROUP_VERSION, resource control should be enabled
    (status = if_available)."""

    with open(bsmr.cwd / ".bsmr", "a") as bsmrconfig:
        bsmrconfig.write("[bsmr_resource_control]\n")
        # Version 1 is the current DAEMON_CGROUP_VERSION, so this should enable.
        bsmrconfig.write("status_if_min_daemon_cgroup_version = 1\n")

    snapshot = await start_daemon_and_get_snapshot(bsmr)
    assert snapshot["allprocs_cgroup"] is not None


@bsmr_test(skip_for_os=["darwin", "windows"], disable_daemon_cgroup=False)
async def test_version_gate_disables_cgroup_when_version_too_high(bsmr: Bsmr) -> None:
    """When status_if_min_daemon_cgroup_version is set to a version higher than
    the binary's DAEMON_CGROUP_VERSION, resource control should remain off."""

    with open(bsmr.cwd / ".bsmr", "a") as bsmrconfig:
        bsmrconfig.write("[bsmr_resource_control]\n")
        # Version 9999 is higher than any DAEMON_CGROUP_VERSION, so this should not enable.
        bsmrconfig.write("status_if_min_daemon_cgroup_version = 9999\n")

    snapshot = await start_daemon_and_get_snapshot(bsmr)
    assert snapshot["allprocs_cgroup"] is None


@bsmr_test(skip_for_os=["darwin", "windows"], disable_daemon_cgroup=False)
async def test_version_gate_not_set_status_off(bsmr: Bsmr) -> None:
    """When status_if_min_daemon_cgroup_version is not set and status is off,
    resource control should be off."""

    with open(bsmr.cwd / ".bsmr", "a") as bsmrconfig:
        bsmrconfig.write("[bsmr_resource_control]\n")
        bsmrconfig.write("status = off\n")

    snapshot = await start_daemon_and_get_snapshot(bsmr)
    assert snapshot["allprocs_cgroup"] is None


# Placeholder for tests to be listed successfully on non-Linux platforms.
async def test_noop() -> None:
    pass


async def start_daemon_and_get_snapshot(bsmr: Bsmr) -> dict[str, typing.Any]:
    await bsmr.targets(":")
    status_result = await bsmr.status("--snapshot")
    status_data = json.loads(status_result.stdout)
    return status_data["snapshot"]
