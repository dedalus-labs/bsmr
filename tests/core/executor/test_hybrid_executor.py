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


from typing import Optional

import pytest
from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.api.bsmr_result import BsmrException, InvocationRecord
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test, env
from bsmr.tests.e2e_util.helper.utils import json_get, random_string, read_what_ran


@bsmr_test()
async def test_hybrid_executor_threshold(bsmr: Bsmr) -> None:
    await bsmr.build(
        "root//executor_threshold_tests/...",
        "-c",
        f"test.cache_buster={random_string()}",
    )
    out = await read_what_ran(bsmr)

    executors = {line["identity"]: line["reproducer"]["executor"] for line in out}
    expected = {
        "root//executor_threshold_tests:big (<unspecified>) (head)": "Local",
        "root//executor_threshold_tests:cp_big (<unspecified>) (cp)": "Local",
        "root//executor_threshold_tests:small (<unspecified>) (head)": "Local",
        "root//executor_threshold_tests:cp_small (<unspecified>) (cp)": "Re",
    }
    assert executors == expected


@bsmr_test()
@pytest.mark.parametrize(
    "low_pass_filter",
    [
        "true",
        "false",
    ],
)
async def test_hybrid_executor_fallbacks(bsmr: Bsmr, low_pass_filter: str) -> None:
    opts = [
        "-c",
        f"test.cache_buster={random_string()}",
        "-c",
        f"test.experimental_low_pass_filter={low_pass_filter}",
    ]

    # Those work as they are allowed to fallback:
    await bsmr.build(
        "root//executor_fallback_tests:local_only",
        "root//executor_fallback_tests:local_only_full_hybrid",
        "root//executor_fallback_tests:remote_only_prefer_local",
        *opts,
    )

    # This one doesn't:
    await expect_failure(
        bsmr.build(
            "root//executor_fallback_tests:local_only_no_fallback",
            *opts,
        )
    )


@bsmr_test()
async def test_hybrid_executor_fallback_preferred_error(bsmr: Bsmr) -> None:
    opts = [
        "-c",
        f"test.cache_buster={random_string()}",
    ]

    await expect_failure(
        bsmr.build(
            "root//executor_fallback_tests:fails_both",
            *opts,
        ),
        stderr_regex="Failed on local",
    )

    await expect_failure(
        bsmr.build(
            "root//executor_fallback_tests:fails_both_prefer_local",
            *opts,
        ),
        stderr_regex="Failed on local",
    )


@bsmr_test()
@pytest.mark.parametrize(
    "target",
    [
        "slower_locally",
        "slower_locally_force_full_hybrid",
    ],
)
async def test_hybrid_executor_cancels_local_execution(bsmr: Bsmr, target: str) -> None:
    await bsmr.build(
        f"root//executor_race_tests:{target}",
        "-c",
        f"test.cache_buster={random_string()}",
    )

    log = (await bsmr.log("show")).stdout.strip().splitlines()
    commands = None

    for line in log:
        commands = commands or json_get(
            line,
            "Event",
            "data",
            "SpanEnd",
            "data",
            "ActionExecution",
            "commands",
        )

    assert commands is not None
    assert len(commands) == 2
    assert commands[0]["status"] == {"Cancelled": {}}
    assert commands[1]["status"] == {"Success": {}}


@bsmr_test()
async def test_hybrid_executor_logging(bsmr: Bsmr) -> None:
    await bsmr.build(
        "root//executor_fallback_tests:local_only",
        "-c",
        f"test.cache_buster={random_string()}",
    )

    log = (await bsmr.log("show")).stdout.strip().splitlines()
    commands = None

    for line in log:
        commands = commands or json_get(
            line,
            "Event",
            "data",
            "SpanEnd",
            "data",
            "ActionExecution",
            "commands",
        )

    assert commands is not None
    assert len(commands) == 2
    assert commands[0]["details"]["signed_exit_code"] != 0
    assert commands[0]["status"] == {"Failure": {}}
    assert commands[1]["details"]["signed_exit_code"] == 0
    assert commands[1]["status"] == {"Success": {}}


@bsmr_test()
@pytest.mark.parametrize(
    "low_pass_filter",
    [
        "true",
        "false",
    ],
)
async def test_hybrid_executor_prefer_local(bsmr: Bsmr, low_pass_filter: str) -> None:
    opts = [
        "-c",
        f"test.cache_buster={random_string()}",
        "-c",
        f"test.experimental_low_pass_filter={low_pass_filter}",
    ]

    # heavyweight_works_only_locally will only succeed if it runs locally, but
    # its weight would normally prevent that from happening. It has
    # prefer_local, so it only works if that results in local execution being
    # attempted.
    #
    # slower_and_works_only_locally will only work locally but it'll fail
    # faster on RE. This means it must not be attempted at al on RE.
    await bsmr.build(
        "root//executor_race_tests:heavyweight_works_only_locally",
        "root//executor_race_tests:slower_and_works_only_locally",
        *opts,
    )

    # Same as above, but with prefer-local on the build command line instead of the command.
    await bsmr.build(
        "root//executor_race_tests:heavyweight_works_only_locally_local_not_preferred",
        "root//executor_race_tests:slower_and_works_only_locally_local_not_preferred",
        "--prefer-local",
        *opts,
    )


@bsmr_test()
async def test_hybrid_executor_prefer_remote_local_fallback(bsmr: Bsmr) -> None:
    opts = [
        "-c",
        f"test.cache_buster={random_string()}",
    ]
    # Local only command that fails with --remote-only, passes with --prefer-remote
    await expect_failure(
        bsmr.build(
            "root//executor_fallback_tests:local_only_full_hybrid",
            "--remote-only",
            *opts,
        ),
        stderr_regex="Failed to build .*local_only_full_hybrid",
    )

    await bsmr.build(
        "root//executor_fallback_tests:local_only_full_hybrid",
        "--prefer-remote",
        *opts,
    )


@bsmr_test()
async def test_hybrid_executor_prefer_remote(bsmr: Bsmr) -> None:
    opts = [
        "-c",
        f"test.cache_buster={random_string()}",
    ]
    # Build execution is sequential and remote first with --prefer-remote
    # using an action that succeeds slowly on RE and fails fast locally
    # that would fail if run concurrently
    await bsmr.build(
        "root//executor_race_tests:slower_remotely",
        "--prefer-remote",
        *opts,
    )


@bsmr_test()
async def test_executor_preference_priority(bsmr: Bsmr) -> None:
    opts = [
        "-c",
        f"test.cache_buster={random_string()}",
    ]

    await bsmr.build(
        "root//executor_preference_tests:",
        "--prefer-remote",
        *opts,
    )


@bsmr_test()
async def test_executor_preference_with_remote_args(bsmr: Bsmr) -> None:
    opts = [
        "-c",
        f"test.cache_buster={random_string()}",
    ]

    await bsmr.build(
        "root//executor_preference_prefer_remote_arg_tests:",
        *opts,
    )


@bsmr_test()
async def test_executor_preference_with_remote_args_and_cli_override(
    bsmr: Bsmr,
) -> None:
    opts = [
        "-c",
        f"test.cache_buster={random_string()}",
    ]

    await expect_failure(
        bsmr.build(
            "root//executor_preference_prefer_remote_arg_tests:",
            # `--prefer-local` takes priority over any `ctx.actions.run()`
            "--prefer-local",
            *opts,
        )
    )


@bsmr_test()
async def test_prefer_local(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.build(
            "root//executor_fallback_tests:local_only_no_fallback",
            "-c",
            f"test.cache_buster={random_string()}",
        )
    )

    await bsmr.build(
        "root//executor_fallback_tests:local_only_no_fallback", "--prefer-local"
    )


@bsmr_test()
async def test_local_only(bsmr: Bsmr) -> None:
    args = [
        "root//executor_fallback_tests:local_only_no_fallback",
        "-c",
        f"test.cache_buster={random_string()}",
    ]

    await expect_failure(bsmr.build(*args))

    await bsmr.build(
        *args,
        "--local-only",
    )


@bsmr_test()
async def test_remote_only(bsmr: Bsmr) -> None:
    args = [
        "root//executor_fallback_tests:remote_only_no_fallback",
        "root//executor_fallback_tests:remote_only_full_hybrid",
        "-c",
        f"test.cache_buster={random_string()}",
    ]

    await expect_failure(bsmr.build(*args))

    await bsmr.build(
        *args,
        "--remote-only",
    )


@bsmr_test()
async def test_build_fails_with_mutually_exclusive_executors(bsmr: Bsmr) -> None:
    with pytest.raises(BsmrException):
        await bsmr.build(
            "--local-only", "--remote-only", "root//executor_threshold_tests/..."
        )


@bsmr_test()
@env("BSMR_OFFLINE_BUILD", "1")
async def test_build_offline(bsmr: Bsmr) -> None:
    await bsmr.build("root//executor_threshold_tests/...")
    out = await read_what_ran(bsmr)

    executors = {line["identity"]: line["reproducer"]["executor"] for line in out}
    expected = {
        "root//executor_threshold_tests:big (<unspecified>) (head)": "Local",
        "root//executor_threshold_tests:cp_big (<unspecified>) (cp)": "Local",
        "root//executor_threshold_tests:small (<unspecified>) (head)": "Local",
        "root//executor_threshold_tests:cp_small (<unspecified>) (cp)": "Local",
    }
    assert executors == expected


@bsmr_test(write_invocation_record=True)
async def test_hybrid_executor_remote_queuing_fallback(bsmr: Bsmr) -> None:
    async def build(
        target: str, *opts: str, env: Optional[dict[str, str]] = None
    ) -> InvocationRecord:
        # kill to update env
        await bsmr.kill()
        res = await bsmr.build(
            f"root//executor_race_tests:{target}",
            "-c",
            f"test.cache_buster={random_string()}",
            *opts,
            env=env,
        )
        return res.invocation_record()

    async def scheduling_mode(bsmr: Bsmr) -> int:
        actions = await read_what_ran(bsmr)
        return actions[0]["scheduling_mode"]

    record = await build("slower_remotely_and_works_on_both_full_hybrid")
    assert record["run_local_count"] == 1
    assert record["run_remote_count"] == 0
    assert record["run_fallback_count"] == 0
    assert await scheduling_mode(bsmr) == "FullHybrid"

    record = await build(
        "slower_remotely_and_works_on_both_fallback_only",
        env={"BSMR_TEST_RE_QUEUE_ESTIMATE_S": "0"},
    )
    assert record["run_local_count"] == 0
    assert record["run_remote_count"] == 1
    assert record["run_fallback_count"] == 0
    assert await scheduling_mode(bsmr) == "Fallback"

    record = await build(
        "slower_remotely_and_works_on_both_fallback_only",
        "-c",
        "build.remote_execution_fallback_on_estimated_queue_time_exceeds_s=10",
        env={"BSMR_TEST_RE_QUEUE_ESTIMATE_S": "100"},
    )
    assert record["run_local_count"] == 1
    assert record["run_remote_count"] == 0
    assert record["run_fallback_count"] == 1
    assert await scheduling_mode(bsmr) == "FallbackReQueueEstimate"
