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

from bsmr.tests.core.common.io.file_watcher import (
    FileWatcherEvent,
    FileWatcherEventType,
    FileWatcherKind,
    FileWatcherProvider,
    get_file_watcher_events,
)
from bsmr.tests.core.common.io.file_watcher_dir_tests import (
    run_create_directory_test,
    run_remove_directory_test,
    run_rename_directory_test,
)
from bsmr.tests.core.common.io.file_watcher_file_tests import (
    run_create_file_test,
    run_modify_file_test,
    run_remove_file_test,
    run_rename_file_test,
    run_replace_file_test,
)
from bsmr.tests.core.common.io.file_watcher_scm_tests import (
    run_checkout_mergebase_changes_test,
    run_checkout_with_mergebase_test,
    run_rebase_with_mergebase_test,
    run_restack_with_mergebase_test,
    setup_file_watcher_scm_test,
)
from bsmr.tests.core.common.io.file_watcher_symlink_tests import (
    run_change_symlink_target_test,
    run_create_symlink_test,
    run_replace_file_with_symlink_test,
)
from bsmr.tests.core.common.io.file_watcher_tests import (
    FileSystemType,
    setup_file_watcher_test,
    verify_results,
)
from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


@bsmr_test(setup_eden=False)
async def test_watchman_create_file_no_eden(bsmr: Bsmr) -> None:
    await run_create_file_test(
        bsmr, FileSystemType.NATIVE, FileWatcherProvider.WATCHMAN
    )


@bsmr_test(setup_eden=True)
async def test_watchman_create_file_eden(bsmr: Bsmr) -> None:
    await run_create_file_test(
        bsmr, FileSystemType.EDEN_FS, FileWatcherProvider.WATCHMAN
    )


@bsmr_test(setup_eden=False)
async def test_watchman_modify_file_no_eden(bsmr: Bsmr) -> None:
    await run_modify_file_test(
        bsmr, FileSystemType.NATIVE, FileWatcherProvider.WATCHMAN
    )


@bsmr_test(setup_eden=True)
async def test_watchman_modify_file_eden(bsmr: Bsmr) -> None:
    await run_modify_file_test(
        bsmr, FileSystemType.EDEN_FS, FileWatcherProvider.WATCHMAN
    )


@bsmr_test(setup_eden=False)
async def test_watchman_remove_file_no_eden(bsmr: Bsmr) -> None:
    await run_remove_file_test(
        bsmr, FileSystemType.NATIVE, FileWatcherProvider.WATCHMAN
    )


@bsmr_test(setup_eden=True)
async def test_watchman_remove_file_eden(bsmr: Bsmr) -> None:
    await run_remove_file_test(
        bsmr, FileSystemType.EDEN_FS, FileWatcherProvider.WATCHMAN
    )


@bsmr_test(setup_eden=False)
async def test_watchman_rename_file_no_eden(bsmr: Bsmr) -> None:
    await run_rename_file_test(
        bsmr, FileSystemType.NATIVE, FileWatcherProvider.WATCHMAN
    )


@bsmr_test(setup_eden=True)
async def test_watchman_rename_file_eden(bsmr: Bsmr) -> None:
    await run_rename_file_test(
        bsmr, FileSystemType.EDEN_FS, FileWatcherProvider.WATCHMAN
    )


# File replace is not supported on Windows
@bsmr_test(setup_eden=False, skip_for_os=["windows"])
async def test_watchman_replace_file_no_eden(bsmr: Bsmr) -> None:
    await run_replace_file_test(
        bsmr, FileSystemType.NATIVE, FileWatcherProvider.WATCHMAN
    )


# File replace is not supported on Windows
@bsmr_test(setup_eden=True, skip_for_os=["windows"])
async def test_watchman_replace_file_eden(bsmr: Bsmr) -> None:
    await run_replace_file_test(
        bsmr, FileSystemType.EDEN_FS, FileWatcherProvider.WATCHMAN
    )


@bsmr_test(setup_eden=False)
async def test_watchman_create_directory_no_eden(bsmr: Bsmr) -> None:
    await run_create_directory_test(
        bsmr, FileSystemType.NATIVE, FileWatcherProvider.WATCHMAN
    )


@bsmr_test(setup_eden=True)
async def test_watchman_create_directory_eden(bsmr: Bsmr) -> None:
    await run_create_directory_test(
        bsmr, FileSystemType.EDEN_FS, FileWatcherProvider.WATCHMAN
    )


@bsmr_test(setup_eden=False)
async def test_watchman_remove_directory_no_eden(bsmr: Bsmr) -> None:
    await run_remove_directory_test(
        bsmr, FileSystemType.NATIVE, FileWatcherProvider.WATCHMAN
    )


@bsmr_test(setup_eden=True)
async def test_watchman_remove_directory_eden(bsmr: Bsmr) -> None:
    await run_remove_directory_test(
        bsmr, FileSystemType.EDEN_FS, FileWatcherProvider.WATCHMAN
    )


@bsmr_test(setup_eden=False)
async def test_watchman_rename_directory_no_eden(bsmr: Bsmr) -> None:
    await run_rename_directory_test(
        bsmr, FileSystemType.NATIVE, FileWatcherProvider.WATCHMAN
    )


@bsmr_test(setup_eden=True)
async def test_watchman_rename_directory_eden(bsmr: Bsmr) -> None:
    await run_rename_directory_test(
        bsmr, FileSystemType.EDEN_FS, FileWatcherProvider.WATCHMAN
    )


@bsmr_test(setup_eden=False)
async def test_watchman_checkout_mergebase_changes_no_eden(bsmr: Bsmr) -> None:
    await run_checkout_mergebase_changes_test(
        bsmr, FileSystemType.NATIVE, FileWatcherProvider.WATCHMAN
    )


@bsmr_test(setup_eden=True)
async def test_watchman_checkout_mergebase_changes_eden(bsmr: Bsmr) -> None:
    await run_checkout_mergebase_changes_test(
        bsmr, FileSystemType.EDEN_FS, FileWatcherProvider.WATCHMAN
    )


@bsmr_test(setup_eden=False)
async def test_watchman_checkout_with_mergebase_no_eden(bsmr: Bsmr) -> None:
    await run_checkout_with_mergebase_test(
        bsmr, FileSystemType.NATIVE, FileWatcherProvider.WATCHMAN
    )


@bsmr_test(setup_eden=True)
async def test_watchman_checkout_with_mergebase_eden(bsmr: Bsmr) -> None:
    await run_checkout_with_mergebase_test(
        bsmr, FileSystemType.EDEN_FS, FileWatcherProvider.WATCHMAN
    )


@bsmr_test(setup_eden=False)
async def test_watchman_rebase_with_mergebase_no_eden(bsmr: Bsmr) -> None:
    await run_rebase_with_mergebase_test(
        bsmr, FileSystemType.NATIVE, FileWatcherProvider.WATCHMAN
    )


@bsmr_test(setup_eden=True)
async def test_watchman_rebase_with_mergebase_eden(bsmr: Bsmr) -> None:
    await run_rebase_with_mergebase_test(
        bsmr, FileSystemType.EDEN_FS, FileWatcherProvider.WATCHMAN
    )


@bsmr_test(setup_eden=False)
async def test_watchman_restack_with_mergebase_no_eden(bsmr: Bsmr) -> None:
    await run_restack_with_mergebase_test(
        bsmr, FileSystemType.NATIVE, FileWatcherProvider.WATCHMAN
    )


@bsmr_test(setup_eden=True)
async def test_watchman_restack_with_mergebase_eden(bsmr: Bsmr) -> None:
    await run_restack_with_mergebase_test(
        bsmr, FileSystemType.EDEN_FS, FileWatcherProvider.WATCHMAN
    )


@bsmr_test(
    setup_eden=True,
    extra_bsmr_config={
        "bsmr": {"disable_watchman_empty_on_fresh_instance": "true"},
    },
)
async def test_watchman_files_report_on_fresh_instance(bsmr: Bsmr) -> None:
    await setup_file_watcher_test(bsmr)
    await setup_file_watcher_scm_test(bsmr)
    await bsmr.kill()

    required = [
        FileWatcherEvent(
            FileWatcherEventType.CREATE, FileWatcherKind.FILE, "root//files/ghi"
        ),
        FileWatcherEvent(
            FileWatcherEventType.CREATE, FileWatcherKind.FILE, "root//files/jkl"
        ),
    ]

    is_fresh_instance, results = await get_file_watcher_events(bsmr)
    assert is_fresh_instance
    verify_results(results, required)


@bsmr_test(setup_eden=True)
async def test_watchman_create_symlink_test_eden(bsmr: Bsmr) -> None:
    await run_create_symlink_test(
        bsmr, FileSystemType.EDEN_FS, FileWatcherProvider.WATCHMAN
    )


@bsmr_test(setup_eden=False)
async def test_watchman_create_symlink_test_no_eden(bsmr: Bsmr) -> None:
    await run_create_symlink_test(
        bsmr, FileSystemType.NATIVE, FileWatcherProvider.WATCHMAN
    )


@bsmr_test(setup_eden=True)
async def test_watchman_replace_file_with_symlink_eden(bsmr: Bsmr) -> None:
    await run_replace_file_with_symlink_test(
        bsmr, FileSystemType.EDEN_FS, FileWatcherProvider.WATCHMAN
    )


@bsmr_test(setup_eden=False)
async def test_watchman_replace_file_with_symlink_no_eden(bsmr: Bsmr) -> None:
    await run_replace_file_with_symlink_test(
        bsmr, FileSystemType.NATIVE, FileWatcherProvider.WATCHMAN
    )


@bsmr_test(setup_eden=True)
async def test_watchman_change_symlink_target_test_eden(bsmr: Bsmr) -> None:
    await run_change_symlink_target_test(
        bsmr, FileSystemType.EDEN_FS, FileWatcherProvider.WATCHMAN
    )


@bsmr_test(setup_eden=False)
async def test_watchman_change_symlink_target_test_no_eden(bsmr: Bsmr) -> None:
    await run_change_symlink_target_test(
        bsmr, FileSystemType.NATIVE, FileWatcherProvider.WATCHMAN
    )
