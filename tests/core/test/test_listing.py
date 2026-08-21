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

from enum import Enum
from typing import Any, Dict, List

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test, env
from bsmr.tests.e2e_util.helper.utils import filter_events, random_string


class TestDiscovery(Enum):
    EXECUTED = 1
    CACHED = 2
    SKIPPED = 3


@bsmr_test()
async def test_discovery_output_dir(bsmr: Bsmr) -> None:
    args = [
        "//:ok",
    ]
    await run_test_and_check_discovery_presence(bsmr, TestDiscovery.EXECUTED, args)

    bad_output_path = (bsmr.cwd / "bsmr-out" / "v2" / "test" / "bsmr-out").resolve()
    assert not bad_output_path.exists()

    discovery_output_path = (
        bsmr.cwd / "bsmr-out" / "v2" / "test" / "discovery"
    ).resolve()
    assert discovery_output_path.exists()


@bsmr_test()
async def test_discovery_cached_on_dice(bsmr: Bsmr) -> None:
    args = [
        "//:ok",
    ]
    await run_test_and_check_discovery_presence(bsmr, TestDiscovery.EXECUTED, args)
    await run_test_and_check_discovery_presence(bsmr, TestDiscovery.SKIPPED, args)


@bsmr_test()
async def test_failed_discovery_not_cached_on_dice(bsmr: Bsmr) -> None:
    args = [
        "//:bad",
    ]
    await expect_failure(
        bsmr.test(*args),
        stderr_regex="Failed to list tests",
    )
    whatran = (await bsmr.log("what-ran")).stdout
    assert "test.discovery" in whatran
    assert "test.run" not in whatran

    await expect_failure(
        bsmr.test(*args),
        stderr_regex="Failed to list tests",
    )
    whatran = (await bsmr.log("what-ran")).stdout
    assert "test.discovery" in whatran


@bsmr_test()
async def test_listing_uncacheable(bsmr: Bsmr) -> None:
    seed = random_string()
    args = [
        "-c",
        f"test.seed={seed}",
        "-c",
        "test.remote_enabled=false",
        "-c",
        "test.local_enabled=true",
        "-c",
        "test.remote_cache_enabled=true",
        "//:listing_uncacheable",
    ]
    # Check it executed locally consistently
    await run_test_and_check_discovery_presence(bsmr, TestDiscovery.EXECUTED, args)
    await bsmr.kill()
    await run_test_and_check_discovery_presence(bsmr, TestDiscovery.EXECUTED, args)
    # Check cache is not uploaded
    cached = await _cache_uploads(bsmr)
    assert len(cached) == 0


@bsmr_test()
async def test_discovery_cached_on_re(bsmr: Bsmr) -> None:
    seed = random_string()
    args = [
        "-c",
        f"test.seed={seed}",
        "-c",
        "test.local_enabled=false",
        "-c",
        "test.remote_enabled=true",
        "-c",
        "test.remote_cache_enabled=true",
        "//:test",
    ]
    await run_test_and_check_discovery_presence(bsmr, TestDiscovery.EXECUTED, args)
    await bsmr.kill()
    await run_test_and_check_discovery_presence(bsmr, TestDiscovery.CACHED, args)
    await bsmr.kill()
    args = [
        "-c",
        f"test.seed={seed}",
        "-c",
        "test.remote_enabled=false",
        "-c",
        "test.local_enabled=true",
        "-c",
        "test.remote_cache_enabled=true",
        "//:test",
    ]
    await run_test_and_check_discovery_presence(bsmr, TestDiscovery.CACHED, args)


@bsmr_test()
@env("BSMR_TEST_SKIP_ACTION_CACHE_WRITE", "true")
async def test_local_discovery_uploaded_to_cache(bsmr: Bsmr) -> None:
    seed = random_string()
    args = [
        "-c",
        f"test.seed={seed}",
        "-c",
        "test.allow_cache_uploads=true",
        "-c",
        "test.remote_cache_enabled=true",
        "//:ok",
    ]
    await run_test_and_check_discovery_presence(bsmr, TestDiscovery.EXECUTED, args)
    await _check_cache_uploaded(bsmr)


async def _check_cache_uploaded(bsmr: Bsmr) -> None:
    result = await _cache_uploads(bsmr)
    assert len(result) == 1
    assert result[0]["success"]


async def _cache_uploads(bsmr: Bsmr) -> List[Dict[str, Any]]:
    return await filter_events(bsmr, "Event", "data", "SpanEnd", "data", "CacheUpload")


async def run_test_and_check_discovery_presence(
    bsmr: Bsmr,
    discovery: TestDiscovery,
    args: List[str],
) -> None:
    await bsmr.test(*args)
    stdout = (await bsmr.log("what-ran")).stdout

    assert "test.run" in stdout
    match discovery:
        case TestDiscovery.EXECUTED:
            for line in stdout.splitlines():
                if "test.discovery" in line:
                    if "cached" in line:
                        raise Exception("test.discovery was cached")
                    else:
                        return
            raise Exception("test.discovery was not skipped")
        case TestDiscovery.CACHED:
            for line in stdout.splitlines():
                if "test.discovery" in line:
                    if "cache" in line:
                        return
                    else:
                        raise Exception("test.discovery was executed")
            raise Exception("test.discovery was not skipped")
        case TestDiscovery.SKIPPED:
            assert "test.discovery" not in stdout
        case _:
            raise Exception("Unexpected discovery type")
