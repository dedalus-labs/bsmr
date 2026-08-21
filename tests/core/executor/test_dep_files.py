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


import hashlib
import typing
from pathlib import Path
from typing import Any

import pytest
from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.api.bsmr_result import BsmrException
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test, env
from bsmr.tests.e2e_util.helper.utils import (
    expect_exec_count,
    filter_events,
    random_string,
    read_what_ran,
)

# Taken from data.proto
ACTION_EXECUTION_KIND_LOCAL = 1
ACTION_EXECUTION_KIND_ACTION_CACHE = 3
ACTION_EXECUTION_KIND_SIMPLE = 4
ACTION_EXECUTION_KIND_LOCAL_DEP_FILE = 7
ACTION_EXECUTION_KIND_REMOTE_DEP_FILE_CACHE = 9
ACTION_EXECUTION_KIND_LOCAL_ACTION_CACHE = 10

CACHE_UPLOAD_REASON_LOCAL_EXECUTION = 0
CACHE_UPLOAD_REASON_DEP_FILE = 1


async def expect_only_dep_file_hit(bsmr: Bsmr) -> None:
    what_ran = await read_what_ran(bsmr)
    assert (
        len([x for x in what_ran if x["reproducer"]["executor"] == "LocalDepFileCache"])
        == 1
    )
    assert len(what_ran) == 1


async def check_execution_kind(
    bsmr: Bsmr,
    expecteds: list[int],
    ignored: typing.Optional[list[int]] = None,
) -> None:
    ignored = ignored or []
    execution_kinds = await filter_events(
        bsmr,
        "Event",
        "data",
        "SpanEnd",
        "data",
        "ActionExecution",
        "execution_kind",
    )
    execution_kinds = [kind for kind in execution_kinds if kind not in ignored]
    assert len(execution_kinds) == len(expecteds)
    for actual, expected in zip(execution_kinds, expecteds):
        assert actual == expected


class MatchDepFilesEvent(typing.NamedTuple):
    remote_cache: bool
    checking_filtered_inputs: bool


async def check_match_dep_files_events(
    bsmr: Bsmr,
    expected_events: list[MatchDepFilesEvent],
) -> None:
    match_dep_files = await filter_events(
        bsmr, "Event", "data", "SpanStart", "data", "MatchDepFiles"
    )
    assert len(match_dep_files) == len(expected_events)

    for match, expected_event in zip(match_dep_files, expected_events):
        assert bool(match["remote_cache"]) == expected_event.remote_cache
        assert (
            bool(match["checking_filtered_inputs"])
            == expected_event.checking_filtered_inputs
        )


async def _get_execution_kind(bsmr: Bsmr) -> int:
    execution_kinds = await filter_events(
        bsmr,
        "Event",
        "data",
        "SpanEnd",
        "data",
        "ActionExecution",
        "execution_kind",
    )
    return execution_kinds[0]


def touch(bsmr: Bsmr, name: str) -> None:
    """
    Append a random string to the marker in the file
    """
    with open(bsmr.cwd / name, encoding="utf-8") as f:
        text = f.read()

    with open(bsmr.cwd / name, "w", encoding="utf-8") as f:
        f.write(text.replace("__MARKER__", f"__MARKER__{random_string()}"))


async def _test_dep_files_impl(bsmr: Bsmr, use_content_based_paths: bool) -> None:
    """Common implementation for dep files tests."""
    # We query cache before we query dep file. Disable remote cache to make
    # sure that for the last build what-ran doesn't return cached entry.
    args = [
        "app:app",
        "--no-remote-cache",
        "-c",
        f"test.use_content_based_paths={str(use_content_based_paths).lower()}",
    ]
    await bsmr.build(*args)
    await expect_exec_count(bsmr, 1)

    touch(bsmr, "app/app.h")
    await bsmr.build(*args)
    await expect_exec_count(bsmr, 1)

    touch(bsmr, "app/app.c")
    await bsmr.build(*args)
    await expect_exec_count(bsmr, 1)

    # //app:app doesn't use other.h and
    # using dep file this should build nothing.
    touch(bsmr, "app/other.h")
    await bsmr.build(*args)
    await expect_only_dep_file_hit(bsmr)
    await check_execution_kind(
        bsmr,
        [ACTION_EXECUTION_KIND_LOCAL_DEP_FILE],
        # A symlinked_dir command was re-run because app/other.h was changed
        ignored=[ACTION_EXECUTION_KIND_SIMPLE],
    )

    # Changing the command line itself should cause a rebuild.
    touch(bsmr, "app/other.h")
    await bsmr.build(*args, "-c", f"test.unused_command_line_param={random_string()}")
    await expect_exec_count(bsmr, 1)


# Flaky because of watchman on mac (and maybe windows)
# Skipping on windows due to gcc dependency
@bsmr_test(data_dir="dep_files", skip_for_os=["darwin", "windows"])
async def test_dep_files_with_content_based_paths(bsmr: Bsmr) -> None:
    await _test_dep_files_impl(bsmr, use_content_based_paths=True)


# Flaky because of watchman on mac (and maybe windows)
# Skipping on windows due to gcc dependency
@bsmr_test(data_dir="dep_files", skip_for_os=["darwin", "windows"])
async def test_dep_files_without_content_based_paths(bsmr: Bsmr) -> None:
    await _test_dep_files_impl(bsmr, use_content_based_paths=False)


async def _test_dep_files_in_same_package_impl(
    bsmr: Bsmr, use_content_based_paths: bool
) -> None:
    def make_args(
        used_input1_contents: str,
        used_input2_contents: str,
        unused_input1_contents: str,
        unused_input2_contents: str,
    ) -> list[str]:
        return [
            "app:simple_dep_file",
            "--no-remote-cache",
            "-c",
            f"test.used_input1_contents={used_input1_contents}",
            "-c",
            f"test.used_input2_contents={used_input2_contents}",
            "-c",
            f"test.unused_input1_contents={unused_input1_contents}",
            "-c",
            f"test.unused_input2_contents={unused_input2_contents}",
            "-c",
            f"test.use_content_based_paths={str(use_content_based_paths).lower()}",
        ]

    used_input1_contents = random_string()
    used_input2_contents = random_string()
    unused_input1_contents = random_string()
    unused_input2_contents = random_string()

    await bsmr.build(
        *make_args(
            used_input1_contents,
            used_input2_contents,
            unused_input1_contents,
            unused_input2_contents,
        )
    )
    await expect_exec_count(bsmr, 1)

    used_input1_contents = random_string()
    await bsmr.build(
        *make_args(
            used_input1_contents,
            used_input2_contents,
            unused_input1_contents,
            unused_input2_contents,
        )
    )
    await expect_exec_count(bsmr, 1)

    used_input2_contents = random_string()
    await bsmr.build(
        *make_args(
            used_input1_contents,
            used_input2_contents,
            unused_input1_contents,
            unused_input2_contents,
        )
    )
    await expect_exec_count(bsmr, 1)

    unused_input1_contents = random_string()
    await bsmr.build(
        *make_args(
            used_input1_contents,
            used_input2_contents,
            unused_input1_contents,
            unused_input2_contents,
        )
    )
    await expect_only_dep_file_hit(bsmr)
    await check_execution_kind(
        bsmr,
        [ACTION_EXECUTION_KIND_LOCAL_DEP_FILE],
        ignored=[ACTION_EXECUTION_KIND_SIMPLE],
    )

    unused_input2_contents = random_string()
    await bsmr.build(
        *make_args(
            used_input1_contents,
            used_input2_contents,
            unused_input1_contents,
            unused_input2_contents,
        )
    )
    await expect_only_dep_file_hit(bsmr)
    await check_execution_kind(
        bsmr,
        [ACTION_EXECUTION_KIND_LOCAL_DEP_FILE],
        ignored=[ACTION_EXECUTION_KIND_SIMPLE],
    )


# Flaky because of watchman on mac (and maybe windows)
# Skipping on windows due to gcc dependency
@bsmr_test(data_dir="dep_files", skip_for_os=["darwin", "windows"])
async def test_dep_files_in_same_package_with_content_based(bsmr: Bsmr) -> None:
    await _test_dep_files_in_same_package_impl(bsmr, use_content_based_paths=True)


@bsmr_test(data_dir="dep_files", skip_for_os=["darwin", "windows"])
async def test_dep_files_in_same_package_without_content_based(bsmr: Bsmr) -> None:
    await _test_dep_files_in_same_package_impl(bsmr, use_content_based_paths=False)


async def _test_dep_files_in_same_dir_impl(
    bsmr: Bsmr, use_content_based_paths: bool
) -> None:
    def make_args(
        used_input_contents: str,
        unused_input_contents: str,
    ) -> list[str]:
        return [
            "app:shared_dir_dep_file",
            "--no-remote-cache",
            "-c",
            f"test.used_input_contents={used_input_contents}",
            "-c",
            f"test.unused_input_contents={unused_input_contents}",
            "-c",
            f"test.use_content_based_paths={str(use_content_based_paths).lower()}",
        ]

    used_input_contents = random_string()
    unused_input_contents = random_string()

    await bsmr.build(
        *make_args(
            used_input_contents,
            unused_input_contents,
        )
    )
    await expect_exec_count(bsmr, 1)

    used_input_contents = random_string()
    await bsmr.build(
        *make_args(
            used_input_contents,
            unused_input_contents,
        )
    )
    await expect_exec_count(bsmr, 1)

    unused_input_contents = random_string()
    await bsmr.build(
        *make_args(
            used_input_contents,
            unused_input_contents,
        )
    )
    await expect_only_dep_file_hit(bsmr)

    await check_execution_kind(
        bsmr,
        [ACTION_EXECUTION_KIND_LOCAL_DEP_FILE],
        ignored=[ACTION_EXECUTION_KIND_SIMPLE],
    )


# Flaky because of watchman on mac (and maybe windows)
# Skipping on windows due to gcc dependency
@bsmr_test(data_dir="dep_files", skip_for_os=["darwin", "windows"])
async def test_dep_files_in_same_dir_with_content_based(bsmr: Bsmr) -> None:
    await _test_dep_files_in_same_dir_impl(bsmr, use_content_based_paths=True)


@bsmr_test(data_dir="dep_files", skip_for_os=["darwin", "windows"])
async def test_dep_files_in_same_dir_without_content_based(bsmr: Bsmr) -> None:
    await _test_dep_files_in_same_dir_impl(bsmr, use_content_based_paths=False)


async def get_cache_queries(bsmr: Bsmr) -> list[dict[str, Any]]:
    return await filter_events(
        bsmr,
        "Event",
        "data",
        "SpanStart",
        "data",
        "ExecutorStage",
        "stage",
        "CacheQuery",
    )


async def check_no_cache_query(bsmr: Bsmr) -> None:
    cache_queries = await get_cache_queries(bsmr)
    assert len(cache_queries) == 0


async def check_cache_query(bsmr: Bsmr) -> None:
    cache_queries = await get_cache_queries(bsmr)
    assert len(cache_queries) == 1


# Skipping on windows due to gcc dependency
@bsmr_test(
    # test uses symlinks that mess up with eden symlink redirection on MacOS
    setup_eden=False,
    data_dir="dep_files",
    skip_for_os=["windows"],
)
async def test_dep_file_hit_identical_action(bsmr: Bsmr) -> None:
    # For actions that have dep files, bsmr will query the local dep file cache to see
    # if an identical action is stored there. Otherwise, it will fall back to an action cache
    # look up (if enabled) and then to the full dep file query.
    # This test builds a target to build up a dep file cache, then builds the target again
    # with a no-op configuration change so that we hit the initial dep file lookup hit case.
    dummy1 = "dummy1"
    await bsmr.build(
        "app:app_with_dummy_config",
        "--local-only",
        "--no-remote-cache",  # Turn off remote cache query so we execute locally
        "-c",
        f"test.dummy_config={dummy1}",
    )
    await check_execution_kind(
        bsmr, [ACTION_EXECUTION_KIND_LOCAL], ignored=[ACTION_EXECUTION_KIND_SIMPLE]
    )

    dummy2 = "dummy2"
    await bsmr.build(
        "app:app_with_dummy_config",
        "--local-only",
        "-c",
        f"test.dummy_config={dummy2}",
    )
    # The result should be served by the local dep file cache BEFORE an action cache lookup
    await check_no_cache_query(bsmr)
    # Ignoring any simple actions because there can be either one or two symlink dir actions,
    # with the same dice key,
    # Not sure why but this feels like a DICE bug triggered by the bsmrconfig change.
    await check_execution_kind(
        bsmr,
        [ACTION_EXECUTION_KIND_LOCAL_ACTION_CACHE],
        ignored=[ACTION_EXECUTION_KIND_SIMPLE],
    )
    # The MatchDepFilesStart span should indicate we only checked the depfile cache once
    await check_match_dep_files_events(
        bsmr, [MatchDepFilesEvent(remote_cache=False, checking_filtered_inputs=False)]
    )


# Reproduces T237527198: changing ActionKey (by registering additional actions
# before the dep-file action during analysis) should not cause a dep-file cache
# miss when the dep-file action itself is identical.
@bsmr_test(
    setup_eden=False,
    data_dir="dep_files",
    skip_for_os=["windows"],
)
async def test_dep_file_hit_with_action_key_change(bsmr: Bsmr) -> None:
    await bsmr.build(
        "app:dep_file_with_preceding_actions",
        "--local-only",
        "--no-remote-cache",
        "-c",
        "test.num_preceding_actions=0",
    )
    await check_execution_kind(
        bsmr,
        [ACTION_EXECUTION_KIND_LOCAL],
        ignored=[ACTION_EXECUTION_KIND_SIMPLE],
    )

    # Add a preceding action, shifting the dep-file action's ActionKey index.
    # The dep-file action itself (command, inputs, outputs) is identical.
    await bsmr.build(
        "app:dep_file_with_preceding_actions",
        "--local-only",
        "--no-remote-cache",
        "-c",
        "test.num_preceding_actions=1",
    )
    # TODO(T237527198): The dep-file action should get a LOCAL_ACTION_CACHE hit
    # here since it is identical, but the ActionKey index shift causes the dep
    # file cache to report "Dep files declaration has changed".
    await check_execution_kind(
        bsmr,
        [ACTION_EXECUTION_KIND_LOCAL],
        ignored=[ACTION_EXECUTION_KIND_SIMPLE],
    )


# Flaky because of watchman on mac (and maybe windows)
# Skipping on windows due to gcc dependency
# This test tombstones the hash of the dep file produced by this action.
@bsmr_test(data_dir="dep_files", skip_for_os=["darwin", "windows"])
@env(
    "BSMR_TEST_TOMBSTONED_DIGESTS",
    "e537c6611d7e2ba1c9b71248f7a0ca506e5a0f9a:78",
)
async def test_dep_files_ignore_missing_digests(bsmr: Bsmr, tmp_path: Path) -> None:
    await bsmr.build("app:app")

    with pytest.raises(BsmrException):  # noqa B908
        dep_file_path = tmp_path / "dep_file"
        await bsmr.build("app:app[dep_file]", f"--out={dep_file_path}")

        # If we get here, that means materialization did not fail.
        with open(dep_file_path, "rb") as f:
            dep_file = f.read()
            dep_file_hash = hashlib.sha1(dep_file).hexdigest()
            dep_file_len = len(dep_file)
            raise Exception(
                f"Misconfigured test, BSMR_TEST_TOMBSTONED_DIGESTS to {dep_file_hash}:{dep_file_len}",
            )

    touch(bsmr, "app/other.h")
    await bsmr.build("app:app")

    await expect_exec_count(bsmr, 1)


@bsmr_test(data_dir="invalid_dep_files")
async def test_invalid_dep_files(bsmr: Bsmr) -> None:
    await bsmr.build(
        "//:lazy",
    )
    # Disable remote cache lookup so we actually check for local dep files
    await expect_failure(
        bsmr.build(
            "//:lazy",
            "-c",
            "test.seed=123",
            "--no-remote-cache",
        ),
        stderr_regex="Invalid line encountered in dep file",
    )

    await bsmr.debug("flush-dep-files")
    await bsmr.build("//:lazy")

    # Disable remote cache lookup so we actually check for local dep files
    await expect_failure(
        bsmr.build(
            "//:eager",
            "--eager-dep-files",
            "--no-remote-cache",
        ),
        stderr_regex="Invalid line encountered in dep file",
    )


@bsmr_test(data_dir="mismatched_outputs_dep_files")
async def test_mismatched_outputs_dep_files(bsmr: Bsmr) -> None:
    await bsmr.build("//:test", "-c", "test.prefix=foo", "-c", "test.suffix=bar")
    # Different output now, even though the command has not changed.
    await bsmr.build("//:test", "-c", "test.prefix=foo/bar", "-c", "test.suffix=")


async def _dep_file_uploads(bsmr: Bsmr) -> list[dict[str, Any]]:
    return await filter_events(
        bsmr, "Event", "data", "SpanEnd", "data", "DepFileUpload"
    )


async def _action_executions(bsmr: Bsmr) -> list[dict[str, Any]]:
    return await filter_events(
        bsmr, "Event", "data", "SpanEnd", "data", "ActionExecution"
    )


async def _dep_file_key_from_executions(bsmr: Bsmr) -> str:
    execs = await _action_executions(bsmr)
    assert len(execs) == 1
    return execs[0]["dep_file_key"]


async def _check_uploaded_dep_file_key(bsmr: Bsmr, dep_file_key: str) -> None:
    # BSMR_TEST_SKIP_ACTION_CACHE_WRITE causes action result writes for dep files to always pass.
    # This is to allow testing without action cache write permission.
    dep_file_uploads = [
        upload for upload in await _dep_file_uploads(bsmr) if upload["success"]
    ]
    assert len(dep_file_uploads) == 1
    uploaded_key = dep_file_uploads[0]["remote_dep_file_key"]
    assert dep_file_key == uploaded_key


@bsmr_test(data_dir="upload_dep_files")
@env("BSMR_LOG", "bsmr_execute_impl::executors::caching=debug")
@env("BSMR_TEST_SKIP_ACTION_CACHE_WRITE", "true")
async def test_re_dep_file_uploads_same_key(bsmr: Bsmr) -> None:
    # Test all the cases where the remote dep file key should stay the same
    target = "root//:dep_files"
    tagged_used_file1 = bsmr.cwd / "used.1"  # Used for depfile 0
    tagged_used_file3 = bsmr.cwd / "used.3"  # Used for depfile 1
    assert tagged_used_file1.exists()
    assert tagged_used_file3.exists()

    target = [
        target,
        "-c",
        "test.allow_dep_file_cache_upload=true",
        "-c",
        f"test.cache_buster={random_string()}",
        "--local-only",
    ]

    # Check that building this target results in a dep file cache upload
    await bsmr.build(*target)

    key = await _dep_file_key_from_executions(bsmr)
    await _check_uploaded_dep_file_key(bsmr, key)

    # Changing a tagged (associated with a dep file) input should not change the key
    # The remote dep file key only tracks the untagged inputs. The dep file cache is for checking whether
    # the output is the same despite a tagged file changing.
    tagged_used_file1.write_text("CHANGE")
    tagged_used_file3.write_text("CHANGE")
    await bsmr.build(*target)
    key_tagged_input_change = await _dep_file_key_from_executions(bsmr)
    await _check_uploaded_dep_file_key(bsmr, key_tagged_input_change)
    assert key == key_tagged_input_change


@bsmr_test(data_dir="upload_dep_files")
@env("BSMR_LOG", "bsmr_execute_impl::executors::caching=debug")
@env("BSMR_TEST_SKIP_ACTION_CACHE_WRITE", "true")
async def test_re_dep_file_uploads_different_key(bsmr: Bsmr) -> None:
    # TODO: Mergebase is currently not set in this test.
    # Include it so we can test for the case where the mergebase differs

    keys_seen = []
    target = "root//:dep_files"
    untagged_file1 = bsmr.cwd / "untagged.1"
    assert untagged_file1.exists()
    targets_file = bsmr.cwd / "TARGETS.fixture"
    assert targets_file.exists()

    target = [
        target,
        "-c",
        "test.allow_dep_file_cache_upload=true",
        "-c",
        f"test.cache_buster={random_string()}",
        "--local-only",
    ]

    # Check that building this target results in a dep file cache upload
    await bsmr.build(*target)
    key = await _dep_file_key_from_executions(bsmr)
    await _check_uploaded_dep_file_key(bsmr, key)
    keys_seen.append(key)

    # Modify the depfile name and check the new key is different
    targets_file.write_text(
        targets_file.read_text().replace(
            '"dep_file_name1",', '"dep_file_name1_modified",'
        )
    )
    await bsmr.build(*target)

    key_different_depfile_name = await _dep_file_key_from_executions(bsmr)
    await _check_uploaded_dep_file_key(bsmr, key_different_depfile_name)
    assert key_different_depfile_name not in keys_seen
    keys_seen.append(key_different_depfile_name)

    # Modify the output name and check the new key is different
    targets_file.write_text(
        targets_file.read_text().replace(
            'out_name = "dep_files_out"', 'out_name = "dep_files_out_changed"'
        )
    )
    await bsmr.build(*target)
    key_different_out_name = await _dep_file_key_from_executions(bsmr)
    await _check_uploaded_dep_file_key(bsmr, key_different_out_name)
    assert key_different_out_name not in keys_seen
    keys_seen.append(key_different_out_name)

    # Modify an untagged input and check the new key is different
    untagged_file1.write_text("CHANGE")
    await bsmr.build(*target)
    key_untagged_input_change = await _dep_file_key_from_executions(bsmr)
    await _check_uploaded_dep_file_key(bsmr, key_untagged_input_change)
    assert key_untagged_input_change not in keys_seen
    keys_seen.append(key_untagged_input_change)


@bsmr_test(data_dir="upload_dep_files")
@env("BSMR_LOG", "bsmr_execute_impl::executors::caching=debug")
@env("BSMR_TEST_SKIP_ACTION_CACHE_WRITE", "true")
async def test_dep_file_does_not_upload_when_allow_cache_upload_is_true(
    bsmr: Bsmr,
) -> None:
    target = [
        "root//:dep_files",
        "-c",
        "test.allow_dep_file_cache_upload=false",
        "-c",
        "test.allow_cache_upload=true",
        "-c",
        f"test.cache_buster={random_string()}",
        "--remote-only",
    ]

    # Check that we don't do a dep file cache upload when allow_dep_file_cache_upload is false,
    # even though allow_cache_upload is true
    await bsmr.build(*target)
    uploads = await _dep_file_uploads(bsmr)
    assert len(uploads) == 0


@bsmr_test(data_dir="upload_dep_files")
@env("BSMR_LOG", "bsmr_execute_impl::executors::caching=debug")
@env("BSMR_TEST_SKIP_ACTION_CACHE_WRITE", "true")
@env("BSMR_TEST_ONLY_REMOTE_DEP_FILE_CACHE", "true")
async def test_only_do_cache_lookup_when_dep_file_upload_is_enabled(
    bsmr: Bsmr,
) -> None:
    target = [
        "root//:dep_files",
        "-c",
        "test.allow_dep_file_cache_upload=false",
        "-c",
        "test.allow_cache_upload=true",
        "-c",
        f"test.cache_buster={random_string()}",
        "--remote-only",
    ]

    # Check that we don't do a dep file cache lookup when allow_dep_file_cache_upload is false
    await bsmr.build(*target)
    await check_no_cache_query(bsmr)

    target = [
        "root//:dep_files",
        "-c",
        "test.allow_dep_file_cache_upload=true",
        "-c",
        "test.allow_cache_upload=true",
        "-c",
        f"test.cache_buster={random_string()}",
        "--remote-only",
    ]

    # Check that we do a dep file cache lookup when allow_dep_file_cache_upload is true
    await bsmr.build(*target)
    await check_cache_query(bsmr)


@bsmr_test(data_dir="upload_dep_files")
@env("BSMR_LOG", "bsmr_execute_impl::executors::caching=debug")
@env("BSMR_TEST_SKIP_ACTION_CACHE_WRITE", "true")
async def test_re_dep_file_remote_upload(bsmr: Bsmr) -> None:
    target = [
        "root//:dep_files",
        "-c",
        "test.allow_dep_file_cache_upload=true",
        "-c",
        f"test.cache_buster={random_string()}",
        "--remote-only",
    ]

    # Check that building on RE results in a dep file cache upload
    await bsmr.build(*target)
    key = await _dep_file_key_from_executions(bsmr)
    await _check_uploaded_dep_file_key(bsmr, key)


@bsmr_test(data_dir="upload_dep_files", write_invocation_record=True)
@env("BSMR_LOG", "bsmr_action_impl=debug,bsmr_execute_impl::executors::caching=debug")
@env("BSMR_TEST_SKIP_ACTION_CACHE_WRITE", "true")
async def test_re_dep_file_cache_hit_upload(bsmr: Bsmr) -> None:
    target = [
        "root//:dep_files",
        "--remote-only",
        "-c",
        # Ensure we don't get a dep file cache hit
        "test.remote_dep_file_cache_enabled=false",
    ]

    # Build on RE to make sure action cache is populated
    await bsmr.build(*target)
    await bsmr.kill()

    # Check for action cache hit and dep file cache upload
    res = await bsmr.build(
        *target,
        "-c",
        "test.allow_dep_file_cache_upload=true",
    )
    what_ran = await read_what_ran(bsmr)
    assert what_ran[0]["reproducer"]["executor"] == "Cache"
    assert len(what_ran) == 1
    key = await _dep_file_key_from_executions(bsmr)
    await _check_uploaded_dep_file_key(bsmr, key)

    invocation_record = res.invocation_record()

    assert invocation_record["dep_file_upload_count"] == 1
    assert (
        invocation_record["dep_file_upload_count"]
        == invocation_record["dep_file_upload_attempt_count"]
    )

    # Simulate 'user' build, with action cache hit from previous build and dep file cache checking enabled.
    await bsmr.clean()
    await bsmr.build(
        "root//:dep_files",
        "--remote-only",
        "-c",
        "test.remote_dep_file_cache_enabled=true",
        "-c",
        "test.allow_dep_file_cache_upload=false",
    )
    await check_execution_kind(bsmr, [ACTION_EXECUTION_KIND_ACTION_CACHE])
    uploads = await _dep_file_uploads(bsmr)
    # Ensure no dep file uploads are attempted for cache hits with dep file cache checking enabled, but dep file uploads disabled.
    assert len(uploads) == 0


@bsmr_test(data_dir="upload_dep_files")
async def test_re_dep_file_uploads_failed_action(bsmr: Bsmr) -> None:
    # If the action failed, we should not attempt to upload to cache even if it's configured to
    target = [
        "root//:dep_files_fail",
        "-c",
        "test.allow_dep_file_cache_upload=true",
    ]
    await expect_failure(
        bsmr.build(
            *target,
            "--no-remote-cache",
            "--local-only",
        ),
        stderr_regex="Failing on purpose",
    )
    # Assert cache upload was not attempted
    what_ran = await read_what_ran(bsmr, "--emit-cache-queries")
    for what in what_ran:
        assert "CacheQuery" != what["reproducer"]["executor"]


async def check_remote_dep_file_cache_query_took_place(bsmr: Bsmr) -> str:
    what_ran = await read_what_ran(bsmr, "--emit-cache-queries")
    assert "CacheQuery" == what_ran[0]["reproducer"]["executor"]
    return what_ran[0]["reproducer"]["details"]["digest"]


@bsmr_test(data_dir="upload_dep_files")
@env(
    "BSMR_LOG",
    "bsmr_execute_impl::executors::caching=debug,bsmr_execute_impl::executors::action_cache=debug,bsmr_action_impl=debug",
)
# Disable the regular action cache query so that we actually hit the remote dep file cache query.
@env("BSMR_TEST_ONLY_REMOTE_DEP_FILE_CACHE", "true")
async def test_re_dep_file_query_change_tagged_unused_file(bsmr: Bsmr) -> None:
    target = "root//:dep_files"
    # Tagged for depfile0, and exists in depfile0
    tagged_used_file1 = bsmr.cwd / "used.1"
    # Tagged for depfile0, but does NOT exist in depfile0
    tagged_unused = bsmr.cwd / "unused.1"
    assert tagged_used_file1.exists()
    assert tagged_unused.exists()

    target_upload_enabled = [
        target,
        "-c",
        "test.allow_dep_file_cache_upload=true",
        "-c",
        "test.cache_buster=tagged_unused_file_test",
        "--local-only",
    ]

    target_upload_enabled_with_action_definition_change = target_upload_enabled + [
        "-c",
        "test.allow_cache_upload=true",
    ]

    # Build it once with cache upload (cache upload will fail locally)
    result = await bsmr.build(*target_upload_enabled, "--no-remote-cache")
    output = result.get_build_report().output_for_target(target).read_text()
    assert output == "used1\nused2\nused3\n"

    # Build the target again. This will either result in one of
    # 1. A remote dep file cache hit and a subsequent dep file validation
    # 2. A remote dep file cache miss, fall back to local execution (local dep file cache is
    #    flushed) This can occur if the action definition changes, because the new remote dep file
    #    can only be uploaded by a job with the correct permissions, so it will run locally until
    #    that takes place.
    await bsmr.debug("flush-dep-files")
    result = await bsmr.build(*target_upload_enabled_with_action_definition_change)
    output = result.get_build_report().output_for_target(target).read_text()
    assert output == "used1\nused2\nused3\n"

    await check_remote_dep_file_cache_query_took_place(bsmr)
    execution_kind = await _get_execution_kind(bsmr)
    was_cache_hit = "Cache hits: 100%" in result.stderr
    assert (
        was_cache_hit and execution_kind == ACTION_EXECUTION_KIND_REMOTE_DEP_FILE_CACHE
    ) or (not was_cache_hit and execution_kind == ACTION_EXECUTION_KIND_LOCAL)
    expected_dep_file_match_events = [
        MatchDepFilesEvent(
            remote_cache=False, checking_filtered_inputs=False
        ),  # Initial local dep file cache lookup for an identical action
    ]

    if execution_kind == ACTION_EXECUTION_KIND_REMOTE_DEP_FILE_CACHE:
        expected_dep_file_match_events.append(
            MatchDepFilesEvent(remote_cache=True, checking_filtered_inputs=True)
        )  # Remote dep file cache hit verification

    # Check the MatchDepFiles events
    await check_match_dep_files_events(bsmr, expected_dep_file_match_events)

    # # Change a file that is tracked by a dep file but shows up as unused, we get a local dep file cache hit
    # # as that is checked first.
    tagged_unused.write_text(random_string())
    result = await bsmr.build(*target_upload_enabled)
    output = result.get_build_report().output_for_target(target).read_text()
    assert output == "used1\nused2\nused3\n"

    execution_kind = await _get_execution_kind(bsmr)
    assert execution_kind == ACTION_EXECUTION_KIND_LOCAL_DEP_FILE

    # Change a file that is tracked by a dep file but shows up as unused, this will again result in one of
    # 1. A remote dep file cache hit and a subsequent dep file validation
    # 2. A remote dep file cache miss, fall back to local execution (local dep file cache is flushed)
    await bsmr.debug("flush-dep-files")
    tagged_unused.write_text(random_string())
    result = await bsmr.build(*target_upload_enabled)
    output = result.get_build_report().output_for_target(target).read_text()
    assert output == "used1\nused2\nused3\n"

    await check_remote_dep_file_cache_query_took_place(bsmr)
    execution_kind = await _get_execution_kind(bsmr)
    was_cache_hit = "Cache hits: 100%" in result.stderr
    assert (
        was_cache_hit and execution_kind == ACTION_EXECUTION_KIND_REMOTE_DEP_FILE_CACHE
    ) or (not was_cache_hit and execution_kind == ACTION_EXECUTION_KIND_LOCAL)

    # Check the MatchDepFiles events
    await check_match_dep_files_events(bsmr, expected_dep_file_match_events)


@bsmr_test(data_dir="upload_dep_files")
@env(
    "BSMR_LOG",
    "bsmr_execute_impl::executors::caching=debug,bsmr_execute_impl::executors::action_cache=debug,bsmr_action_impl=debug",
)
# Disable the regular action cache query so that we actually hit the remote dep file cache query.
@env("BSMR_TEST_ONLY_REMOTE_DEP_FILE_CACHE", "true")
async def test_re_dep_file_query_change_tagged_used_file(bsmr: Bsmr) -> None:
    target = "root//:dep_files"
    # Tagged for depfile0, and exists in depfile0
    tagged_used_file1 = bsmr.cwd / "used.1"
    # Tagged for depfile0, but does NOT exist in depfile0
    tagged_unused = bsmr.cwd / "unused.1"
    assert tagged_used_file1.exists()
    assert tagged_unused.exists()

    target_upload_enabled = [
        target,
        "-c",
        "test.allow_dep_file_cache_upload=true",
        "--local-only",
    ]

    # Build it once with cache upload (cache upload will fail locally)
    result = await bsmr.build(*target_upload_enabled, "--no-remote-cache")
    output = result.get_build_report().output_for_target(target).read_text()
    assert output == "used1\nused2\nused3\n"

    # Change a file that is tracked by a dep file and shows up as used (ends up listed in the dep file).
    # Build the target again. This will either result in one of
    # 1. A remote dep file cache hit and a subsequent dep file validation (which fails)
    # 2. A remote dep file cache miss, fall back to local execution (local dep file cache is flushed)
    # Either way, it should be executed locally
    await bsmr.debug("flush-dep-files")
    used1_modified_str = f"used1({random_string()})"
    tagged_used_file1.write_text(f"{used1_modified_str}\n")
    result = await bsmr.build(*target_upload_enabled)
    await check_remote_dep_file_cache_query_took_place(bsmr)
    await check_execution_kind(bsmr, [ACTION_EXECUTION_KIND_LOCAL])
    output = result.get_build_report().output_for_target(target).read_text()
    assert output == f"{used1_modified_str}\nused2\nused3\n"


# Flaky because of watchman on mac (and maybe windows)
# Skipping on windows due to gcc dependency
@bsmr_test(data_dir="dep_files", skip_for_os=["darwin", "windows"])
async def test_flush_dep_files(bsmr: Bsmr) -> None:
    # Make sure that we build locally
    args = ["app:app", "--no-remote-cache", "--local-only"]
    await bsmr.build(*args)
    await expect_exec_count(bsmr, 1)

    await bsmr.debug("flush-dep-files", "--retain-local")

    # //app:app doesn't use other.h and
    # dep file should still be present
    # since we retained local dep files
    touch(bsmr, "app/other.h")
    await bsmr.build(*args)
    await expect_only_dep_file_hit(bsmr)

    await bsmr.debug("flush-dep-files")

    # all dep files are gone, so we have
    # to rebuild.
    touch(bsmr, "app/other.h")
    await bsmr.build(*args)
    await expect_exec_count(bsmr, 1)


async def run_test_input_cannot_be_normalized(
    bsmr: Bsmr, allow_soft_errors: bool
) -> None:
    target = "root//:input_cannot_be_normalized"
    tagged_unused = bsmr.cwd / "unused.1"
    assert tagged_unused.exists()

    # We query cache before we query dep file. Disable remote cache to make
    # sure that for the last build what-ran doesn't return cached entry.
    args = [target, "--no-remote-cache"]
    await bsmr.build(*args)
    await expect_exec_count(bsmr, 1)

    # We should get a dep file cache hit, but we don't because the input cannot be normalized.
    tagged_unused.write_text(random_string())
    if allow_soft_errors:
        await bsmr.build(*args)
        await expect_exec_count(bsmr, 1)
    else:
        await expect_failure(
            bsmr.build(*args),
            stderr_regex="Path.*cannot be normalized for dep-files because it has two path segments that look like a content-based hash!",
        )


@bsmr_test(data_dir="upload_dep_files", allow_soft_errors=False)
async def test_input_cannot_be_normalized_and_hard_error(bsmr: Bsmr) -> None:
    await run_test_input_cannot_be_normalized(bsmr, False)


@bsmr_test(data_dir="upload_dep_files", allow_soft_errors=True)
async def test_input_cannot_be_normalized(bsmr: Bsmr) -> None:
    await run_test_input_cannot_be_normalized(bsmr, True)


@bsmr_test(data_dir="invalid_dep_files")
async def test_two_outputs_tagged_as_dep_file(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.build("root//:two_outputs_tagged_as_dep_file"),
        stderr_regex="`dep_files` value with key `deps` has an invalid count of associated outputs. Expected 1, got 2",
    )


@bsmr_test(data_dir="invalid_dep_files")
async def test_no_outputs_tagged_as_dep_file(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.build("root//:no_outputs_tagged_as_dep_file"),
        stderr_regex="`dep_files` value with key `deps` has an invalid count of associated outputs. Expected 1, got 0",
    )


@bsmr_test(data_dir="invalid_dep_files")
async def test_same_tag_for_multiple_labels(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.build("root//:same_tag_for_multiple_labels"),
        stderr_regex="`dep_files` with keys `deps` and `deps2` are using the same tag",
    )


@bsmr_test(data_dir="invalid_dep_files")
async def test_input_tagged_multiple_times(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.build("root//:input_tagged_multiple_times"),
        stderr_regex="Dep-files input.*input_tagged_multiple_times.txt.*is tagged with multiple tags relevant for dep-files: `deps1` and `deps2`",
    )
