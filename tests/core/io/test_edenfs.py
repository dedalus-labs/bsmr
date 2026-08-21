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

import os
import re
import subprocess
from datetime import datetime
from pathlib import Path
from typing import Optional

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
from bsmr.tests.e2e_util.helper.utils import filter_events


@bsmr_test(setup_eden=True)
async def test_edenfs_create_file(bsmr: Bsmr) -> None:
    await run_create_file_test(
        bsmr, FileSystemType.EDEN_FS, FileWatcherProvider.EDEN_FS
    )


@bsmr_test(setup_eden=True)
async def test_edenfs_modify_file(bsmr: Bsmr) -> None:
    await run_modify_file_test(
        bsmr, FileSystemType.EDEN_FS, FileWatcherProvider.EDEN_FS
    )


@bsmr_test(setup_eden=True)
async def test_edenfs_remove_file(bsmr: Bsmr) -> None:
    await run_remove_file_test(
        bsmr, FileSystemType.EDEN_FS, FileWatcherProvider.EDEN_FS
    )


@bsmr_test(setup_eden=True)
async def test_edenfs_rename_file(bsmr: Bsmr) -> None:
    await run_rename_file_test(
        bsmr, FileSystemType.EDEN_FS, FileWatcherProvider.EDEN_FS
    )


# File replace is not supported on Windows
@bsmr_test(setup_eden=True, skip_for_os=["windows"])
async def test_edenfs_replace_file(bsmr: Bsmr) -> None:
    await run_replace_file_test(
        bsmr, FileSystemType.EDEN_FS, FileWatcherProvider.EDEN_FS
    )


@bsmr_test(setup_eden=True)
async def test_edenfs_create_directory(bsmr: Bsmr) -> None:
    await run_create_directory_test(
        bsmr, FileSystemType.EDEN_FS, FileWatcherProvider.EDEN_FS
    )


@bsmr_test(setup_eden=True)
async def test_edenfs_remove_directory(bsmr: Bsmr) -> None:
    await run_remove_directory_test(
        bsmr, FileSystemType.EDEN_FS, FileWatcherProvider.EDEN_FS
    )


@bsmr_test(setup_eden=True)
async def test_edenfs_rename_directory(bsmr: Bsmr) -> None:
    await run_rename_directory_test(
        bsmr, FileSystemType.EDEN_FS, FileWatcherProvider.EDEN_FS
    )


@bsmr_test(setup_eden=True)
async def test_edenfs_checkout_mergebase_changes(bsmr: Bsmr) -> None:
    await run_checkout_mergebase_changes_test(
        bsmr, FileSystemType.EDEN_FS, FileWatcherProvider.EDEN_FS
    )


@bsmr_test(setup_eden=True)
async def test_edenfs_checkout_with_mergebase(bsmr: Bsmr) -> None:
    await run_checkout_with_mergebase_test(
        bsmr, FileSystemType.EDEN_FS, FileWatcherProvider.EDEN_FS
    )


@bsmr_test(setup_eden=True)
async def test_edenfs_rebase_with_mergebase(bsmr: Bsmr) -> None:
    await run_rebase_with_mergebase_test(
        bsmr, FileSystemType.EDEN_FS, FileWatcherProvider.EDEN_FS
    )


@bsmr_test(setup_eden=True)
async def test_edenfs_restack_with_mergebase(bsmr: Bsmr) -> None:
    await run_restack_with_mergebase_test(
        bsmr, FileSystemType.EDEN_FS, FileWatcherProvider.EDEN_FS
    )


@bsmr_test(setup_eden=True)
async def test_edenfs_create_symlink_test(bsmr: Bsmr) -> None:
    await run_create_symlink_test(
        bsmr, FileSystemType.EDEN_FS, FileWatcherProvider.EDEN_FS
    )


@bsmr_test(setup_eden=True)
async def test_edenfs_replace_file_with_symlink_test(bsmr: Bsmr) -> None:
    await run_replace_file_with_symlink_test(
        bsmr, FileSystemType.EDEN_FS, FileWatcherProvider.EDEN_FS
    )


@bsmr_test(setup_eden=True)
async def test_edenfs_change_symlink_target_test(bsmr: Bsmr) -> None:
    await run_change_symlink_target_test(
        bsmr, FileSystemType.EDEN_FS, FileWatcherProvider.EDEN_FS
    )


@bsmr_test(setup_eden=True)
async def test_edenfs_truncate_journal(bsmr: Bsmr) -> None:
    await setup_file_watcher_test(bsmr)
    subprocess.run(["edenfsctl", "debug", "flush_journal"], cwd=bsmr.cwd)

    is_fresh_instance, _ = await get_file_watcher_events(bsmr)
    assert is_fresh_instance


@bsmr_test(setup_eden=True)
async def test_edenfs_file_watcher_stats(bsmr: Bsmr) -> None:
    await setup_file_watcher_test(bsmr)

    file_stats = await filter_events(
        bsmr,
        "Event",
        "data",
        "SpanEnd",
        "data",
        "FileWatcher",
        "stats",
    )

    file_stats = file_stats[0]
    assert file_stats["fresh_instance"]
    assert file_stats["branched_from_revision"] is not None
    assert file_stats["branched_from_revision_timestamp"] is not None
    # we don't have global revision for test repo
    assert file_stats["branched_from_global_rev"] is None
    assert file_stats["eden_version"] is not None


@bsmr_test(setup_eden=True)
async def test_edenfs_files_report_on_fresh_instance(bsmr: Bsmr) -> None:
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


@bsmr_test(
    setup_eden=True,
    # the test has subproject and creates bsmr-out in subproject,
    # when we setup eden we assume that bsmr-out is in project root dir
    # and redirect only that bsmr-out and not bsmr-out in subproject.
    # So, ignore soft errors that bsmr-out isn't redirected
    allow_soft_errors=True,
    # test is flaky on windows
    skip_for_os=["windows"],
)
async def test_edenfs_changes_in_subproject(bsmr: Bsmr) -> None:
    cwd = Path("subproject")
    await bsmr.targets("cell//:", rel_cwd=cwd)

    with open(bsmr.cwd / "subproject" / "abc", "a"):
        pass

    _, results = await get_file_watcher_events(
        bsmr, target_pattern="cell//:", rel_cwd=cwd
    )
    required = [
        FileWatcherEvent(
            FileWatcherEventType.CREATE,
            FileWatcherKind.FILE,
            "cell//abc",
        ),
    ]
    verify_results(results, required)


@bsmr_test(
    setup_eden=True,
    # the test has subproject and creates bsmr-out in subproject,
    # when we setup eden we assume that bsmr-out is in project root dir
    # and redirect only that bsmr-out and not bsmr-out in subproject.
    # So, ignore soft errors that bsmr-out isn't redirected
    allow_soft_errors=True,
    # test is flaky on windows
    skip_for_os=["windows"],
)
async def test_edenfs_changes_outside_subproject(bsmr: Bsmr) -> None:
    cwd = Path("subproject")
    await bsmr.targets("cell//:", rel_cwd=cwd)

    with open(bsmr.cwd / "subproject" / "abc", "a"):
        pass

    with open(bsmr.cwd / "cde", "a"):
        pass

    _, results = await get_file_watcher_events(
        bsmr, target_pattern="cell//:", rel_cwd=cwd
    )
    required = [
        FileWatcherEvent(
            FileWatcherEventType.CREATE,
            FileWatcherKind.FILE,
            "cell//abc",
        ),
    ]
    verify_results(results, required)


def create_and_commit(cwd: Path, path: str) -> str:
    os.makedirs(os.path.dirname(cwd / path), exist_ok=True)
    with open(cwd / path, "a"):
        pass

    subprocess.run(["sl", "commit", "--addremove", "-m", path], cwd=cwd)
    return subprocess.check_output(["sl", "whereami"], cwd=cwd).decode()


@bsmr_test(setup_eden=True)
async def test_edenfs_checkout_dir_changes(bsmr: Bsmr) -> None:
    # We want to create the following commit tree,
    # so when we move between f2 and f1 the mergbase doesn't change:
    #
    # o  xxxxxxxxf3 main
    # │
    # │ o  xxxxxxxxf2
    # ├─╯  files/d2/f2
    # │
    # o  xxxxxxxxf1

    f1 = create_and_commit(bsmr.cwd, "files/d1/f1")
    f2 = create_and_commit(bsmr.cwd, "files/d2/f2")
    subprocess.run(["sl", "co", f1], cwd=bsmr.cwd)
    subprocess.run(["sl", "bookmark", "main"], cwd=bsmr.cwd, check=True)
    create_and_commit(bsmr.cwd, "files/d3/f3")
    subprocess.run(["sl", "co", f2], cwd=bsmr.cwd)

    await bsmr.targets("root//:")
    subprocess.run(["sl", "co", f1], cwd=bsmr.cwd)
    is_fresh_instance, results = await get_file_watcher_events(bsmr)
    required = [
        FileWatcherEvent(
            FileWatcherEventType.DELETE, FileWatcherKind.FILE, "root//files/d2/f2"
        ),
        FileWatcherEvent(
            FileWatcherEventType.DELETE, FileWatcherKind.DIRECTORY, "root//files/d2"
        ),
        FileWatcherEvent(
            FileWatcherEventType.MODIFY, FileWatcherKind.DIRECTORY, "root//files"
        ),
    ]
    assert not is_fresh_instance
    verify_results(results, required)


@bsmr_test(setup_eden=True)
async def test_edenfs_directory_rename(bsmr: Bsmr) -> None:
    (bsmr.cwd / "d1").mkdir()
    (bsmr.cwd / "d1" / "TARGETS.fixture").touch()
    await bsmr.targets("root//d1:")

    (bsmr.cwd / "d1").rename(bsmr.cwd / "d2")
    # FIXME(JakobDegen): Bug: This directory doesn't exist.
    # Note: Also repros with watchman
    await bsmr.targets("root//d1:")


# Dir replace is not supported on Windows
@bsmr_test(setup_eden=True, skip_for_os=["windows"])
async def test_edenfs_directory_replace(bsmr: Bsmr) -> None:
    await setup_file_watcher_test(bsmr)
    (bsmr.cwd / "d1").mkdir()
    (bsmr.cwd / "d2").mkdir()
    # it's only possible to replace a dir
    # if newname exists and is an empty directory
    (bsmr.cwd / "d1").rename(bsmr.cwd / "d2")

    # we should get `create` for the newname
    # and `delete` for the oldname
    required = [
        FileWatcherEvent(
            FileWatcherEventType.CREATE, FileWatcherKind.DIRECTORY, "root//d2"
        ),
        FileWatcherEvent(
            FileWatcherEventType.DELETE, FileWatcherKind.DIRECTORY, "root//d1"
        ),
    ]

    _, results = await get_file_watcher_events(bsmr)
    verify_results(results, required)


@bsmr_test(setup_eden=True)
async def test_edenfs_duplicated_notifications(bsmr: Bsmr) -> None:
    await setup_file_watcher_test(bsmr)

    with open(bsmr.cwd / "files" / "abc", "a") as f:
        f.write("test")

    with open(bsmr.cwd / "files" / "bcd", "a"):
        pass

    with open(bsmr.cwd / "files" / "abc", "a") as f:
        f.write("test1")

    _, results = await get_file_watcher_events(bsmr)
    # eden watcher doesn't report duplicates
    assert results == [
        FileWatcherEvent(
            FileWatcherEventType.MODIFY, FileWatcherKind.FILE, "root//files/abc"
        ),
        FileWatcherEvent(
            FileWatcherEventType.CREATE, FileWatcherKind.FILE, "root//files/bcd"
        ),
    ]


def get_eden_version(bsmr: Bsmr) -> Optional[datetime]:
    eden_out = subprocess.check_output(["eden", "-v"], cwd=bsmr.cwd).decode()
    match = re.search(r"Running:\s*(\d+)", eden_out)
    if match:
        return datetime.strptime(match.group(1).strip(), "%Y%m%d")
    else:
        # if eden is not running, take installed version
        match = re.search(r"Installed:\s*(\d+)", eden_out)
        if match:
            return datetime.strptime(match.group(1).strip(), "%Y%m%d")
    return None


@bsmr_test(setup_eden=True)
async def test_edenfs_hg_clean_update(bsmr: Bsmr) -> None:
    await setup_file_watcher_test(bsmr)

    with open(bsmr.cwd / "files" / "abc", "a") as f:
        f.write("test")

    _, results = await get_file_watcher_events(bsmr)
    required = [
        FileWatcherEvent(
            FileWatcherEventType.MODIFY,
            FileWatcherKind.FILE,
            "root//files/abc",
        ),
    ]
    verify_results(results, required)

    subprocess.run(["hg", "up", "-C", "."], cwd=bsmr.cwd)

    eden_version = get_eden_version(bsmr)
    assert eden_version is not None, "Failed to get eden version"

    expected_result = []
    if eden_version >= datetime(2025, 6, 12, 0, 0, 0):
        # hg up -C . deletes all the changes and eden should report modification in `root//files/abc`
        expected_result.append(
            FileWatcherEvent(
                FileWatcherEventType.MODIFY,
                FileWatcherKind.FILE,
                "root//files/abc",
            )
        )

    _, results = await get_file_watcher_events(bsmr)
    assert results == expected_result
