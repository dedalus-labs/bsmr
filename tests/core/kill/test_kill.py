#!/usr/bin/env fbpython
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


import platform

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test, env


_BSMR_TEST_DECORATOR = bsmr_test(
    # On windows, we get an error of form
    # "The process cannot access the file because it is being used by another process"
    # when trying to kill the daemon with sqlite states enabled. This is most
    # likely because we don't kill all child processes of the daemon and so the sqlite process
    # is still running and accessing the sqlite db file when being killed. Given this is a
    # pre-existing issue, we disable sqlite state on windows for now.
    extra_bsmr_config={
        "bsmr": {
            "sqlite_materializer_state": "false",
            "sqlite_incremental_state": "false",
        },
    }
    if platform.system() == "Windows"
    else {},
)


@_BSMR_TEST_DECORATOR
@env("BSMR_TEST_FAIL_BSMRD_AUTH", "true")
async def test_kill_error(bsmr: Bsmr) -> None:
    # Performing a build should fail, since we will not be able to authenticate to the
    # bsmr daemon
    await expect_failure(bsmr.build("//:abc"), stderr_regex="injected auth error")

    # Kill should succeed, even though we cannot authenticate to the daemon
    await bsmr.kill()


@_BSMR_TEST_DECORATOR
@env("BSMR_TEST_FAIL_BSMRD_AUTH", "true")
async def test_clean_error(bsmr: Bsmr) -> None:
    # Performing a build should fail, since we will not be able to authenticate to the
    # bsmr daemon
    await expect_failure(bsmr.build("//:abc"), stderr_regex="injected auth error")

    # Clean should succeed, even though we cannot authenticate to the daemon
    await bsmr.clean()
