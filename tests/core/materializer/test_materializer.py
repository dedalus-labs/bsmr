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


import sys
from pathlib import Path

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test, env
from bsmr.tests.e2e_util.helper.utils import filter_events


def watchman_dependency_linux_only() -> bool:
    return sys.platform == "linux"


def replace_in_file(old: str, new: str, file: Path, encoding: str = "utf-8") -> None:
    with open(file, encoding=encoding) as f:
        file_content = f.read()
    file_content = file_content.replace(old, new)
    with open(file, "w", encoding=encoding) as f:
        f.write(file_content)


@bsmr_test(data_dir="modify_deferred_materialization")
async def test_modify_input_source(bsmr: Bsmr) -> None:
    await bsmr.build("//:urandom_dep")

    targets_file = bsmr.cwd / "TARGETS.fixture"

    # Change the label in Targets.
    replace_in_file("__NOT_A_REAL_LABEL__", "bsmr_test_local_exec", file=targets_file)

    await bsmr.build("//:urandom_dep")


@bsmr_test(
    data_dir="modify_deferred_materialization_deps",
    skip_for_os=["windows"],  # TODO(marwhal): Fix and enable on Windows
)
async def test_modify_dep_materialization(bsmr: Bsmr) -> None:
    target = "//:check"

    # Build, expect the symlink to work. We'll materialize the first time.

    result = await bsmr.build(target)
    with open(result.get_build_report().output_for_target(target)) as f:
        assert f.read().strip() == "TEXT"

    # Build again, expect the symlink to work. We'll materialize just deps.

    with open(bsmr.cwd / "text", "w", encoding="utf-8") as f:
        f.write("TEXT2")

    result = await bsmr.build(target)
    with open(result.get_build_report().output_for_target(target)) as f:
        assert f.read().strip() == "TEXT2"

    # Build again, expect the symlink to work. We'll materialize just deps
    # again. However this time our state is a little different since the
    # previous future was a check-deps only future.

    with open(bsmr.cwd / "text", "w", encoding="utf-8") as f:
        f.write("TEXT3")

    result = await bsmr.build(target)
    with open(result.get_build_report().output_for_target(target)) as f:
        assert f.read().strip() == "TEXT3"


@bsmr_test(
    data_dir="deferred_materializer_matching_artifact_optimization",
)
@env("BSMR_LOG", "bsmr_execute_impl::materializers=trace")
async def test_matching_artifact_optimization(bsmr: Bsmr) -> None:
    target = "root//:copy"
    result = await bsmr.build(target)
    # Check output is correctly materialized
    assert result.get_build_report().output_for_target(target).exists()

    # In this case, modifying `hidden` does not change the output, so the output should not
    # need to be rematerialized
    with open(bsmr.cwd / "hidden", "w", encoding="utf-8") as f:
        f.write("HIDDEN2")

    result = await bsmr.build(target)
    # Check output still exists
    assert result.get_build_report().output_for_target(target).exists()
    # Check that materializer did not report any rematerialization
    assert "already materialized, updating deps only" in result.stderr
    assert "materialize artifact" not in result.stderr

    # In this case, modifying `src` changes the output, so the output should be rematerialized
    with open(bsmr.cwd / "src", "w", encoding="utf-8") as f:
        f.write("SRC2")

    result = await bsmr.build(target)
    # Check output still exists
    output = result.get_build_report().output_for_target(target)
    assert output.exists()
    with open(output) as f:
        assert f.read().strip() == "SRC2"


@bsmr_test(
    data_dir="deferred_materializer_matching_artifact_optimization",
)
async def test_cache_directory_cleanup(bsmr: Bsmr) -> None:
    # sqlite materializer state is already enabled
    cache_dir = Path(bsmr.cwd, "bsmr-out", "v2", "cache")
    materializer_state_dir = cache_dir / "materializer_state"
    materializer_state_dir.mkdir(parents=True)
    incremental_state_dir = cache_dir / "incremental_state"
    incremental_state_dir.mkdir(parents=True)
    command_hashes_dir = cache_dir / "command_hashes"
    command_hashes_dir.mkdir(parents=True)

    # Need to run a command to start the daemon.
    await bsmr.audit_config()

    cache_dir_listing = sorted(list(cache_dir.iterdir()))
    assert cache_dir_listing == [incremental_state_dir, materializer_state_dir]

    await bsmr.kill()
    disable_sqlite_materializer_state(bsmr)
    await bsmr.audit_config()

    cache_dir_listing = list(cache_dir.iterdir())
    assert cache_dir_listing == [incremental_state_dir]


@bsmr_test(
    data_dir="deferred_materializer_matching_artifact_optimization",
)
@env("BSMR_LOG", "bsmr_execute_impl::materializers=trace")
async def test_sqlite_materializer_state_matching_artifact_optimization(
    bsmr: Bsmr,
) -> None:
    # sqlite materializer state is already enabled
    target = "root//:copy"
    res = await bsmr.build(target)
    # Check output is correctly materialized
    assert res.get_build_report().output_for_target(target).exists()

    await bsmr.kill()

    res = await bsmr.build(target)
    # Check that materializer did not report any rematerialization
    assert "already materialized, updating deps only" in res.stderr, res.stderr
    assert "materialize artifact" not in res.stderr

    await bsmr.kill()

    # In this case, modifying `src` changes the output, so the output should be rematerialized
    with open(bsmr.cwd / "src", "w", encoding="utf-8") as f:
        f.write("SRC2")

    res = await bsmr.build(target)
    # Check output still exists
    output = res.get_build_report().output_for_target(target)
    assert output.exists()
    with open(output) as f:
        assert f.read().strip() == "SRC2"


@bsmr_test(
    data_dir="deferred_materializer_matching_artifact_optimization",
)
@env("BSMR_LOG", "bsmr_execute_impl::materializers=trace")
async def test_download_file_sqlite_matching_artifact_optimization(
    bsmr: Bsmr,
) -> None:
    # sqlite materializer state is already enabled
    target = "root//:download"
    res = await bsmr.build(target)
    # Check output is correctly materialized
    assert res.get_build_report().output_for_target(target).exists()

    await bsmr.kill()

    res = await bsmr.build(target)
    # Check that materializer did not report any rematerialization
    assert "already materialized, updating deps only" in res.stderr, res.stderr
    assert "materialize artifact" not in res.stderr


@bsmr_test(
    data_dir="deferred_materializer_matching_artifact_optimization",
)
@env("BSMR_LOG", "bsmr_execute_impl::materializers=trace")
async def test_sqlite_materializer_state_disabled(
    bsmr: Bsmr,
) -> None:
    disable_sqlite_materializer_state(bsmr)

    target = "root//:copy"
    result = await bsmr.build(target)
    # Check output is correctly materialized
    assert result.get_build_report().output_for_target(target).exists()

    await bsmr.kill()

    result = await bsmr.build(target)
    # Check that materializer did have to rematerialize the same artifact
    assert "already materialized, updating deps only" not in result.stderr
    assert "materialize artifact" in result.stderr


@bsmr_test(
    data_dir="deferred_materializer_matching_artifact_optimization",
)
@env("BSMR_LOG", "bsmr_execute_impl::materializers=trace")
async def test_sqlite_materializer_state_bsmrconfig_version_change(
    bsmr: Bsmr,
) -> None:
    # sqlite materializer state is already enabled
    target = "root//:copy"
    result = await bsmr.build(target)
    # Check output is correctly materialized
    assert result.get_build_report().output_for_target(target).exists()

    await bsmr.kill()

    # Bump the bsmrconfig version of sqlite materializer state to invalidate the existing sqlite db
    replace_in_file(
        "sqlite_materializer_state_version = 0",
        "sqlite_materializer_state_version = 1",
        bsmr.cwd / ".bsmr",
    )

    # just starting the bsmr daemon should delete the sqlite materializer state
    await bsmr.audit_config()


@bsmr_test(
    data_dir="modify_deferred_materialization_deps",
    skip_for_os=["windows"],
)
async def test_materialization_spans_have_parent_id(bsmr: Bsmr) -> None:
    """Materialization spans should be parented to the span that triggered them,
    not appear as root spans with parent_id == 0."""
    await bsmr.build("//:check")

    materialization_events = await filter_events(
        bsmr,
        "Event",
        "data",
        "SpanStart",
        "data",
        "Materialization",
        return_root=True,
    )

    assert len(materialization_events) > 0, "Expected at least one Materialization span"
    for event in materialization_events:
        assert event["Event"]["parent_id"] != 0, (
            f"Materialization span has parent_id == 0 (no parent): {event}"
        )


@bsmr_test(
    data_dir="modify_deferred_materialization_deps",
    skip_for_os=["windows"],
)
async def test_materializer_command_events_have_parent_id(bsmr: Bsmr) -> None:
    """MaterializerCommand instant events emitted on the synchronous command
    processing thread should be parented to the span that sent the command,
    not appear as root events with parent_id == 0.  This requires
    verbose_materializer_event_log = true in .bsmr."""
    await bsmr.build("//:check")

    command_events = await filter_events(
        bsmr,
        "Event",
        "data",
        "Instant",
        "data",
        "MaterializerCommand",
        return_root=True,
    )

    assert len(command_events) > 0, (
        "Expected at least one MaterializerCommand instant event "
        "(is verbose_materializer_event_log enabled?)"
    )
    for event in command_events:
        assert event["Event"]["parent_id"] != 0, (
            f"MaterializerCommand event has parent_id == 0 (no parent): {event}"
        )


def disable_sqlite_materializer_state(bsmr: Bsmr) -> None:
    config_file = bsmr.cwd / ".bsmr"
    replace_in_file(
        "sqlite_materializer_state = true",
        "sqlite_materializer_state = false",
        file=config_file,
    )


@bsmr_test(
    data_dir="modify_deferred_materialization_deps",
    skip_for_os=["windows"],  # TODO(marwhal): Fix and enable on Windows
)
async def test_debug_materialize(bsmr: Bsmr) -> None:
    result = await bsmr.build("//:remote_text", "--materializations=None")
    out = result.get_build_report().output_for_target(
        "root//:remote_text", rel_path=True
    )
    assert not Path(bsmr.cwd, out).exists()

    await bsmr.debug("materialize", str(out))
    assert Path(bsmr.cwd, out).exists()
