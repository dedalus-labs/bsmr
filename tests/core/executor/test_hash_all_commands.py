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


import pytest
from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test, env
from bsmr.tests.e2e_util.helper.utils import expect_exec_count


@bsmr_test()
@env("BSMR_LOG", "bsmr_action_impl::actions::impls::run::dep_files=trace")
@pytest.mark.parametrize(
    "local_only",
    [
        "true",
        "false",
    ],
)
async def test_hash_all_commands(bsmr: Bsmr, local_only: str) -> None:
    # Expecting a rebuild since the command wasn't hashed previously.
    await bsmr.build(
        "//:test",
        "-c",
        "test.seed=123",
        "-c",
        f"test.local_only={local_only}",
    )
    await expect_exec_count(bsmr, 1)
    # Disable remote cache lookup so we actually utilize the local depfile cache for this test
    # No longer expecting a rebuild.
    res = await bsmr.build(
        "//:test",
        "-c",
        "test.seed=456",
        "-c",
        f"test.local_only={local_only}",
        "--no-remote-cache",
    )
    await expect_exec_count(bsmr, 0)

    # Check that we're matching on just the directory here
    assert "Command line and directory have not changed" in res.stderr


@bsmr_test()
async def test_hash_all_commands_key_change(bsmr: Bsmr) -> None:
    # Expecting a rebuild since the command wasn't hashed previously.
    await bsmr.build(
        "//:test",
        "-c",
        "test.param=123",
        "-c",
        "test.category=cat1",
    )
    await expect_exec_count(bsmr, 1)

    # Again expecting a rebuild as the action doesn't match and also it's not
    # even the same action.
    await bsmr.build(
        "//:test",
        "-c",
        "test.param=456",
        "-c",
        "test.category=cat2",
    )
    await expect_exec_count(bsmr, 1)

    # Again expecting a rebuild as the action once again doesn't match, but
    # while is mismatches with the 2nd action (which had a differet key),
    # it does match the 1st action (which had the same key). Since the 2nd
    # action clobbered the output, however, this *must* re-run.
    await bsmr.build(
        "//:test",
        "-c",
        "test.param=123",
        "-c",
        "test.category=cat1",
    )
    await expect_exec_count(bsmr, 1)


@bsmr_test()
async def test_hash_all_commands_key_change_deps(bsmr: Bsmr) -> None:
    # Expecting a rebuild since the command wasn't hashed previously.
    await bsmr.build(
        "//:symlink_test",
        "-c",
        "test.param=123",
        "-c",
        "test.category=cat1",
    )
    await expect_exec_count(bsmr, 1)

    # Again expecting a rebuild because the cache key is different anyway.
    await bsmr.build(
        "//:symlink_test",
        "-c",
        "test.param=456",
        "-c",
        "test.category=cat2",
    )
    await expect_exec_count(bsmr, 1)

    # Not actually expecting a rebuild this time, because the symlink
    # output is unchanged.
    res = await bsmr.build(
        "//:symlink_test",
        "-c",
        "test.param=123",
        "-c",
        "test.category=cat1",
    )
    await expect_exec_count(bsmr, 0)

    # But we should have the right output here
    build_report = res.get_build_report()
    output = build_report.output_for_target("//:symlink_test")
    assert output.read_text().rstrip() == "123"
