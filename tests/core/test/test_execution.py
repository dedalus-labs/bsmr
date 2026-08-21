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

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test
from bsmr.tests.e2e_util.helper.utils import random_string, read_what_ran


@bsmr_test()
async def test_stable_action_digest_with_deterministic_paths(bsmr: Bsmr) -> None:
    args = [
        "-c",
        "test.local_enabled=false",
        "-c",
        "test.remote_enabled=true",
        "//:test",
    ]

    await bsmr.test(*args)
    first_what_ran = await read_what_ran(bsmr)
    first_digests = [
        entry["reproducer"]["details"]["digest"]
        for entry in first_what_ran
        if entry["reason"] == "test.run"
    ]
    assert len(first_digests) == 1, "Expected one test.run entry"

    await bsmr.test(*args)
    second_what_ran = await read_what_ran(bsmr)
    second_digests = [
        entry["reproducer"]["details"]["digest"]
        for entry in second_what_ran
        if entry["reason"] == "test.run"
    ]
    assert len(second_digests) == 1, "Expected one test.run entry"

    assert first_digests[0] == second_digests[0], (
        f"Test action digests differ between runs: {first_digests[0]} vs {second_digests[0]}"
    )


@bsmr_test()
async def test_stress_runs_have_different_action_digests(bsmr: Bsmr) -> None:
    await bsmr.test(
        "-c",
        "test.local_enabled=false",
        "-c",
        "test.remote_enabled=true",
        "//:test",
        "--",
        "--stress-runs",
        "2",
    )
    what_ran = await read_what_ran(bsmr)
    test_runs = [entry for entry in what_ran if entry["reason"] == "test.run"]
    assert len(test_runs) == 2, (
        f"Expected exactly 2 test.run entries for stress runs, got {len(test_runs)}"
    )

    digests = [entry["reproducer"]["details"]["digest"] for entry in test_runs]
    assert digests[0] != digests[1], (
        f"Stress run action digests should differ but were both: {digests[0]}"
    )


@bsmr_test()
async def test_remote_test_execution_cached(bsmr: Bsmr) -> None:
    args = [
        "-c",
        "test.local_enabled=false",
        "-c",
        "test.remote_enabled=true",
        "//:cacheable_test",
    ]

    await bsmr.test(*args)

    await bsmr.test(*args)
    second_what_ran = await read_what_ran(bsmr, "--emit-cache-queries")
    second_test_runs = [
        entry
        for entry in second_what_ran
        if entry["reason"] == "test.run"
        and entry.get("reproducer", {}).get("executor") == "Cache"
    ]
    assert len(second_test_runs) == 1, (
        f"Expected exactly one cached test.run entry, got {len(second_test_runs)}"
    )


@bsmr_test()
async def test_remote_test_execution_not_cached_for_stress_runs(bsmr: Bsmr) -> None:
    args = [
        "-c",
        "test.local_enabled=false",
        "-c",
        "test.remote_enabled=true",
        "//:cacheable_test",
        "--",
        "--stress-runs",
        "2",
    ]

    await bsmr.test(*args)

    await bsmr.test(*args)
    what_ran = await read_what_ran(bsmr)
    test_runs = [entry for entry in what_ran if entry["reason"] == "test.run"]
    assert len(test_runs) == 2, (
        f"Expected exactly 2 test.run entries for stress runs, got {len(test_runs)}"
    )

    # Stress runs disable caching — even on the second invocation, both runs
    # should execute remotely rather than hitting the cache.
    for entry in test_runs:
        executor = entry.get("reproducer", {}).get("executor", "")
        assert executor == "Re", (
            f"Expected Re executor for stress runs, got: {executor}"
        )


@bsmr_test()
async def test_local_test_execution_not_cached(bsmr: Bsmr) -> None:
    seed = random_string()
    args = [
        "-c",
        "test.local_enabled=true",
        "-c",
        "test.remote_enabled=false",
        "-c",
        f"test.seed={seed}",
        "//:cacheable_test",
    ]

    await bsmr.test(*args)

    await bsmr.test(*args)
    second_what_ran = await read_what_ran(bsmr)
    second_test_runs = [
        entry for entry in second_what_ran if entry["reason"] == "test.run"
    ]
    assert len(second_test_runs) == 1, (
        f"Expected exactly one test.run entry, got {len(second_test_runs)}"
    )
    assert second_test_runs[0]["reproducer"]["executor"] == "Local", (
        "Expected test to run locally, not be cached!"
    )


@bsmr_test()
async def test_remote_test_execution_not_cached_with_no_remote_cache(
    bsmr: Bsmr,
) -> None:
    args = [
        "-c",
        "test.local_enabled=false",
        "-c",
        "test.remote_enabled=true",
        "--no-remote-cache",
        "//:cacheable_test",
    ]

    await bsmr.test(*args)

    await bsmr.test(*args)
    second_what_ran = await read_what_ran(bsmr)
    second_test_runs = [
        entry for entry in second_what_ran if entry["reason"] == "test.run"
    ]
    assert len(second_test_runs) == 1, (
        f"Expected exactly one test.run entry, got {len(second_test_runs)}"
    )
    assert second_test_runs[0]["reproducer"]["executor"] == "Re", (
        "Expected test to run remotely, not be cached!"
    )


@bsmr_test()
async def test_remote_test_execution_not_cached_with_disable_flag(
    bsmr: Bsmr,
) -> None:
    args = [
        "-c",
        "test.local_enabled=false",
        "-c",
        "test.remote_enabled=true",
        "//:cacheable_test",
        "--",
        "--disable-test-execution-caching",
    ]

    await bsmr.test(*args)

    await bsmr.test(*args)
    second_what_ran = await read_what_ran(bsmr)
    second_test_runs = [
        entry for entry in second_what_ran if entry["reason"] == "test.run"
    ]
    assert len(second_test_runs) == 1, (
        f"Expected exactly one test.run entry, got {len(second_test_runs)}"
    )
    assert second_test_runs[0]["reproducer"]["executor"] == "Re", (
        "Expected test to run remotely, not be cached!"
    )
