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


from bsmr.tests.core.common.io.file_watcher import FileWatcherProvider
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
)
from bsmr.tests.core.common.io.file_watcher_symlink_tests import (
    run_change_symlink_target_test,
    run_create_symlink_test,
    run_replace_file_with_symlink_test,
)
from bsmr.tests.core.common.io.file_watcher_tests import FileSystemType
from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


@bsmr_test(setup_eden=False)
async def test_fs_hash_cralwer_create_file(bsmr: Bsmr) -> None:
    await run_create_file_test(
        bsmr, FileSystemType.NATIVE, FileWatcherProvider.FS_HASH_CRAWLER
    )


@bsmr_test(setup_eden=False)
async def test_fs_hash_cralwer_modify_file(bsmr: Bsmr) -> None:
    await run_modify_file_test(
        bsmr, FileSystemType.NATIVE, FileWatcherProvider.FS_HASH_CRAWLER
    )


@bsmr_test(setup_eden=False)
async def test_fs_hash_cralwer_remove_file(bsmr: Bsmr) -> None:
    await run_remove_file_test(
        bsmr, FileSystemType.NATIVE, FileWatcherProvider.FS_HASH_CRAWLER
    )


@bsmr_test(setup_eden=False)
async def test_fs_hash_cralwer_rename_file(bsmr: Bsmr) -> None:
    await run_rename_file_test(
        bsmr, FileSystemType.NATIVE, FileWatcherProvider.FS_HASH_CRAWLER
    )


# File replace is not supported on Windows
@bsmr_test(setup_eden=False, skip_for_os=["windows"])
async def test_fs_hash_cralwer_replace_file(bsmr: Bsmr) -> None:
    await run_replace_file_test(
        bsmr, FileSystemType.NATIVE, FileWatcherProvider.FS_HASH_CRAWLER
    )


@bsmr_test(setup_eden=False)
async def test_fs_hash_cralwer_create_directory(bsmr: Bsmr) -> None:
    await run_create_directory_test(
        bsmr, FileSystemType.NATIVE, FileWatcherProvider.FS_HASH_CRAWLER
    )


@bsmr_test(setup_eden=False)
async def test_fs_hash_cralwer_remove_directory(bsmr: Bsmr) -> None:
    await run_remove_directory_test(
        bsmr, FileSystemType.NATIVE, FileWatcherProvider.FS_HASH_CRAWLER
    )


@bsmr_test(setup_eden=False)
async def test_fs_hash_cralwer_rename_directory(bsmr: Bsmr) -> None:
    await run_rename_directory_test(
        bsmr, FileSystemType.NATIVE, FileWatcherProvider.FS_HASH_CRAWLER
    )


@bsmr_test(setup_eden=False)
async def test_fs_hash_cralwer_checkout_mergebase_changes(bsmr: Bsmr) -> None:
    await run_checkout_mergebase_changes_test(
        bsmr, FileSystemType.NATIVE, FileWatcherProvider.FS_HASH_CRAWLER
    )


@bsmr_test(setup_eden=False)
async def test_fs_hash_cralwer_checkout_with_mergebase(bsmr: Bsmr) -> None:
    await run_checkout_with_mergebase_test(
        bsmr, FileSystemType.NATIVE, FileWatcherProvider.FS_HASH_CRAWLER
    )


@bsmr_test(setup_eden=False)
async def test_fs_hash_cralwer_rebase_with_mergebase(bsmr: Bsmr) -> None:
    await run_rebase_with_mergebase_test(
        bsmr, FileSystemType.NATIVE, FileWatcherProvider.FS_HASH_CRAWLER
    )


@bsmr_test(setup_eden=False)
async def test_fs_hash_cralwer_restack_with_mergebase(bsmr: Bsmr) -> None:
    await run_restack_with_mergebase_test(
        bsmr, FileSystemType.NATIVE, FileWatcherProvider.FS_HASH_CRAWLER
    )


@bsmr_test(setup_eden=False)
async def test_fs_hash_crawler_create_symlink_test(bsmr: Bsmr) -> None:
    await run_create_symlink_test(
        bsmr, FileSystemType.NATIVE, FileWatcherProvider.FS_HASH_CRAWLER
    )


@bsmr_test(setup_eden=False)
async def test_fs_hash_crawler_replace_file_with_symlink_test(bsmr: Bsmr) -> None:
    await run_replace_file_with_symlink_test(
        bsmr, FileSystemType.NATIVE, FileWatcherProvider.FS_HASH_CRAWLER
    )


@bsmr_test(setup_eden=False)
async def test_fs_hash_crawler_change_symlink_target_test(bsmr: Bsmr) -> None:
    await run_change_symlink_target_test(
        bsmr, FileSystemType.NATIVE, FileWatcherProvider.FS_HASH_CRAWLER
    )
