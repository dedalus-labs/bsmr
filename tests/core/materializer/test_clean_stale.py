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


import re
import shutil
import time
from datetime import datetime, timedelta
from pathlib import Path

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test, env


def modify_acess_times_updates(bsmr: Bsmr, new_status: str) -> None:
    config_file = bsmr.cwd / ".bsmr"
    replace_in_file(
        "update_access_times = full",
        f"update_access_times = {new_status}",
        file=config_file,
    )


def replace_in_file(old: str, new: str, file: Path, encoding: str = "utf-8") -> None:
    with open(file, encoding=encoding) as f:
        file_content = f.read()
    file_content = file_content.replace(old, new)
    with open(file, "w", encoding=encoding) as f:
        f.write(file_content)


@bsmr_test()
@env("BSMR_LOG", "bsmr_execute_impl::materializers=trace")
async def test_artifact_access_time(bsmr: Bsmr) -> None:
    # drop microseconds to match 1s precision from materializer
    start = datetime.utcnow().replace(microsecond=0)
    target = "root//:copy"
    result = await bsmr.build(target)
    assert result.get_build_report().output_for_target(target).exists()

    async def audit_materialized() -> list[str]:
        return list(
            filter(
                lambda x: "\tmaterialized" in x,
                (await bsmr.audit("deferred-materializer", "list"))
                .stdout.strip()
                .splitlines(),
            )
        )

    def parse_entry_ts(entry: str) -> datetime:
        match = re.search("\tmaterialized \\(ts=([^ ,]*)", entry)
        assert match
        timestamp = datetime.strptime(match.group(1), "%Y-%m-%dT%H:%M:%SZ")
        assert timestamp, match.group(1)
        return timestamp

    materialized_entries = await audit_materialized()
    assert len(materialized_entries) == 1
    materialized_time = parse_entry_ts(materialized_entries[0])
    assert materialized_time >= start

    # Check that access time set after daemon restart
    await bsmr.kill()
    materialized_entries = await audit_materialized()
    assert len(materialized_entries) == 1
    materialized_time = parse_entry_ts(materialized_entries[0])
    assert materialized_time >= start

    # Check that access time is updated following build
    time.sleep(1)
    await bsmr.build(target)

    materialized_entries = await audit_materialized()

    assert len(materialized_entries) == 1
    access_time = parse_entry_ts(materialized_entries[0])
    assert access_time > materialized_time


@bsmr_test()
@env("BSMR_LOG", "bsmr_execute_impl::materializers=trace")
@env("BSMR_ACCESS_TIME_UPDATE_MAX_BUFFER_SIZE", "0")
async def test_clean_stale_artifacts(bsmr: Bsmr) -> None:
    target_1 = "root//:copy"
    result_1 = await bsmr.build(target_1)
    output_1 = result_1.get_build_report().output_for_target(target_1)

    # ensure timestamp is after first materialization and before second
    # (resolution for access timestamps is 1 second)
    time.sleep(1)
    after_first_build = int(time.time())
    time.sleep(1)

    target_2 = "root//:copy_2"
    result_2 = await bsmr.build(target_2)
    output_2 = result_2.get_build_report().output_for_target(target_2)

    # Check output is correctly materialized
    assert output_1.exists()
    assert output_2.exists()

    await bsmr.clean(f"--keep-since-time={after_first_build}")
    # Check output_1 still materialized, it's stale but it was built by running daemon
    assert output_1.exists()

    await bsmr.kill()
    res = await bsmr.clean(f"--keep-since-time={after_first_build}")
    # Check output_1 was cleaned because it's stale and not declared by running daemon
    assert "1 stale artifact" in res.stderr and "4 bytes cleaned" in res.stderr
    assert not output_1.exists()
    assert output_2.exists()

    future_time = int((datetime.now() + timedelta(weeks=7)).timestamp())

    # Check that a previously materialized output re-declared by new daemon is not cleaned
    await bsmr.build(target_2)
    await bsmr.clean(f"--keep-since-time={future_time}")
    assert output_2.exists()

    # Check that setting keep-since-time in the future cleans non-active artifacts
    await bsmr.kill()
    await bsmr.clean(f"--keep-since-time={future_time}")
    assert "1 stale artifact" in res.stderr and "4 bytes cleaned" in res.stderr
    assert not output_2.exists()


@bsmr_test()
@env("BSMR_LOG", "bsmr_execute_impl::materializers=trace")
async def test_clean_stale_artifact_dir(bsmr: Bsmr) -> None:
    target_1 = "root//:copy_dir"
    result_1 = await bsmr.build(target_1)
    output_1 = result_1.get_build_report().output_for_target(target_1)
    assert output_1.exists()
    await bsmr.kill()
    future_time = int((datetime.now() + timedelta(weeks=7)).timestamp())
    res = await bsmr.clean(f"--keep-since-time={future_time}")
    assert "4 bytes cleaned" in res.stderr
    assert not output_1.exists()
    # NOTE: Currently we require clean twice to delete empty dirs, which is ...
    # probably fine.
    await bsmr.clean(f"--keep-since-time={future_time}")
    output_parent = output_1.parent
    while not output_parent.exists():
        output_parent = output_parent.parent
    assert output_parent.parts[-3:] == ("bsmr-out", "v2", "art")


@bsmr_test()
@env("BSMR_ACCESS_TIME_UPDATE_MAX_BUFFER_SIZE", "0")
async def test_clean_stale_output_empty(bsmr: Bsmr) -> None:
    output = await bsmr.clean("--stale")
    assert "Nothing to clean" in output.stderr


@bsmr_test()
@env("BSMR_LOG", "bsmr_execute_impl::materializers=trace")
@env("BSMR_ACCESS_TIME_UPDATE_MAX_BUFFER_SIZE", "0")
async def test_clean_stale_actions(bsmr: Bsmr) -> None:
    query_res = await bsmr.cquery("root//...")
    targets = [
        target.split(" ")[0] for target in query_res.stdout.split("\n") if target
    ]

    outputs = []
    for target in targets:
        res = await bsmr.build(target)
        output = res.get_build_report().outputs_for_target(target)
        outputs += output

    assert len(outputs) >= len(targets)
    for output in outputs:
        assert output.exists()

    await bsmr.clean("--stale")
    for output in outputs:
        assert output.exists()


@bsmr_test()
async def test_clean_stale_declared(bsmr: Bsmr) -> None:
    await bsmr.build("//declared:declared")
    await bsmr.kill()

    # Drop the state. The path exists on disk.
    shutil.rmtree(bsmr.cwd / "bsmr-out/default/cache/materializer_state")

    # Build again, start by declaring, then clean, then require locally.
    await bsmr.build("//declared:remote")
    await bsmr.clean("--stale")
    await bsmr.build("//declared:local")


@bsmr_test()
async def test_clean_stale_scheduled(bsmr: Bsmr) -> None:
    # Need to write to .bsmr instead of passing cmd line args because
    # the config used when creating daemon state does not include cmd line args (but maybe it should).
    config_file = bsmr.cwd / ".bsmr.local"
    with open(config_file, "w") as f:
        f.write(
            """
[bsmr]
clean_stale_enabled = true
clean_stale_artifact_ttl_hours = 0
clean_stale_start_offset_hours = 0
# 0.0001h = 360ms
clean_stale_period_hours = 0.0001
        """
        )

    # Just test that a clean runs if enabled via config.
    # Build a target, output is stale immediately but won't be cleaned until restart.
    result = await bsmr.build("root//:copy")
    output = result.get_build_report().output_for_target("root//:copy")
    assert output.exists()
    await bsmr.kill()
    # Create a new daemon and build something else (could be any command that starts a daemon).
    await bsmr.build("//declared:declared")
    # Wait for at least one clean to run (but should have finished multiple cleans).
    time.sleep(3)
    # Original output should be cleaned.
    assert not output.exists()


@bsmr_test(skip_for_os=["windows"])
async def test_clean_stale_scheduled_high_disk_usage(bsmr: Bsmr) -> None:
    # Need to write to .bsmr instead of passing cmd line args because
    # the config used when creating daemon state does not include cmd line args (but maybe it should).
    config_file = bsmr.cwd / ".bsmr.local"
    with open(config_file, "w") as f:
        f.write(
            """
[bsmr]
clean_stale_enabled = true
clean_stale_artifact_ttl_hours = 8
clean_stale_start_offset_hours = 0
# 0.0001h = 360ms
clean_stale_period_hours = 0.0001
clean_stale_low_disk_threshold = 100.0
clean_stale_low_disk_artifact_ttl_hours = 0.0
        """
        )

    # Just test that a clean runs if enabled via config.
    # Build a target, output is stale immediately but won't be cleaned until restart.
    result = await bsmr.build("root//:copy")
    output = result.get_build_report().output_for_target("root//:copy")
    assert output.exists()
    await bsmr.kill()
    # Create a new daemon and build something else (could be any command that starts a daemon).
    await bsmr.build("//declared:declared")
    # Wait for at least one clean to run (but should have finished multiple cleans).
    time.sleep(3)
    # Original output should be cleaned.
    assert not output.exists()


@bsmr_test(skip_for_os=["windows"])
async def test_clean_stale_scheduled_adaptive_high_disk_usage(bsmr: Bsmr) -> None:
    # Threshold of 100.0 guarantees free disk % is always "below" it, so the
    # adaptive loop must promote retained, non-active artifacts to stale even
    # though the regular ttl (8h) would have kept them.
    config_file = bsmr.cwd / ".bsmr.local"
    with open(config_file, "w") as f:
        f.write(
            """
[bsmr]
clean_stale_enabled = true
clean_stale_artifact_ttl_hours = 8
clean_stale_start_offset_hours = 0
# 0.0001h = 360ms
clean_stale_period_hours = 0.0001
clean_stale_low_disk_threshold = 100.0
clean_stale_low_disk_adaptive_enabled = true
clean_stale_low_disk_adaptive_min_ttl_hours = 0
        """
        )

    result = await bsmr.build("root//:copy")
    output = result.get_build_report().output_for_target("root//:copy")
    assert output.exists()
    await bsmr.kill()
    await bsmr.build("//declared:declared")
    time.sleep(3)
    assert not output.exists()


@bsmr_test(skip_for_os=["windows"])
async def test_clean_stale_scheduled_adaptive_threshold_not_tripped(bsmr: Bsmr) -> None:
    # Threshold of 0.0 guarantees free disk % is always above it, so the
    # adaptive loop must never engage and the retained artifact survives.
    config_file = bsmr.cwd / ".bsmr.local"
    with open(config_file, "w") as f:
        f.write(
            """
[bsmr]
clean_stale_enabled = true
clean_stale_artifact_ttl_hours = 8
clean_stale_start_offset_hours = 0
# 0.0001h = 360ms
clean_stale_period_hours = 0.0001
clean_stale_low_disk_threshold = 0.0
clean_stale_low_disk_adaptive_enabled = true
clean_stale_low_disk_adaptive_min_ttl_hours = 0
        """
        )

    result = await bsmr.build("root//:copy")
    output = result.get_build_report().output_for_target("root//:copy")
    assert output.exists()
    await bsmr.kill()
    await bsmr.build("//declared:declared")
    time.sleep(3)
    assert output.exists()


@bsmr_test(skip_for_os=["windows"])
async def test_clean_stale_scheduled_adaptive_min_ttl_protects_recent(
    bsmr: Bsmr,
) -> None:
    # Threshold of 100.0 always trips adaptive promotion, but the freshly
    # built artifact is well within the 24h adaptive min-TTL floor — it must
    # survive even though disk pressure persists.
    config_file = bsmr.cwd / ".bsmr.local"
    with open(config_file, "w") as f:
        f.write(
            """
[bsmr]
clean_stale_enabled = true
clean_stale_artifact_ttl_hours = 8
clean_stale_start_offset_hours = 0
# 0.0001h = 360ms
clean_stale_period_hours = 0.0001
clean_stale_low_disk_threshold = 100.0
clean_stale_low_disk_adaptive_enabled = true
clean_stale_low_disk_adaptive_min_ttl_hours = 24
        """
        )

    result = await bsmr.build("root//:copy")
    output = result.get_build_report().output_for_target("root//:copy")
    assert output.exists()
    await bsmr.kill()
    await bsmr.build("//declared:declared")
    time.sleep(3)
    assert output.exists()


@bsmr_test(skip_for_os=["windows"])
async def test_clean_stale_cli_adaptive_promotes_retained(bsmr: Bsmr) -> None:
    # `--stale=10000d` alone would not clean a freshly-built artifact, but
    # `--adaptive-low-disk-threshold=100.0` always trips the adaptive branch
    # (free disk % is always <= 100%) and `--adaptive-min-ttl=0s` protects
    # nothing, so the retained, non-active artifact must be promoted to stale
    # and removed.
    result = await bsmr.build("root//:copy")
    output = result.get_build_report().output_for_target("root//:copy")
    assert output.exists()
    await bsmr.kill()
    # New daemon — original artifact is retained but no longer active.
    await bsmr.build("//declared:declared")
    res = await bsmr.clean(
        "--stale=10000d",
        "--adaptive-low-disk-threshold=100.0",
        "--adaptive-min-ttl=0s",
    )
    assert "Adaptive low-disk promotion enabled at 100%" in res.stderr
    assert not output.exists()


@bsmr_test(skip_for_os=["windows"])
async def test_clean_stale_cli_adaptive_min_ttl_protects_recent(bsmr: Bsmr) -> None:
    # Adaptive is tripped (threshold=100%), but `--adaptive-min-ttl=24h`
    # protects every retained artifact accessed within the last 24h, so the
    # freshly-built output survives.
    result = await bsmr.build("root//:copy")
    output = result.get_build_report().output_for_target("root//:copy")
    assert output.exists()
    await bsmr.kill()
    await bsmr.build("//declared:declared")
    await bsmr.clean(
        "--stale=10000d",
        "--adaptive-low-disk-threshold=100.0",
        "--adaptive-min-ttl=24h",
    )
    assert output.exists()
