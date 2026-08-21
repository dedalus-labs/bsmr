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
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test, env


@bsmr_test(
    setup_eden=False,
    extra_bsmr_config={"bsmr": {"file_watcher": "edenfs"}},
)
@env("BSMR_HARD_ERROR", "false")
async def test_watchman_fallback(bsmr: Bsmr) -> None:
    res = await bsmr.targets("root//:")
    # fallback to watchman
    assert "Watchman fresh instance" in res.stderr


@bsmr_test(
    setup_eden=False,
    extra_bsmr_config={"bsmr": {"file_watcher": "edenfs"}},
)
async def test_eden_fail(bsmr: Bsmr) -> None:
    res = await expect_failure(bsmr.targets("root//:"))
    assert "Couldn't initiate connection to Eden" in res.stderr
