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


from __future__ import annotations

import json

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test, env

# Note: for test scenarios where we want to ensure the `cpe` crate reports no
# vpnless support, we have to define the env var but =0. Otherwise these
# tests will erroneously fail on macOS.


@bsmr_test()
@env("CPE_RUST_X2P_SUPPORTS_VPNLESS", "0")
@env("CPE_RUST_X2P_HTTP1_PROXY_PORT", "5555")
async def test_vpnless_disabled_by_host(bsmr: Bsmr) -> None:
    # Get a daemon to start
    await bsmr.build()
    result = await bsmr.status()
    status = json.loads(result.stdout)
    assert not status["supports_vpnless"], (
        "vpnless should be disabled by non-supporting host"
    )


@bsmr_test()
@env("CPE_RUST_X2P_SUPPORTS_VPNLESS", "1")
# Need to set this so Windows doesn't go down the unix socket codepath.
@env("CPE_RUST_X2P_HTTP1_PROXY_PORT", "5555")
async def test_vpnless_enabled(bsmr: Bsmr) -> None:
    # Get a daemon to start
    await bsmr.build()
    result = await bsmr.status()
    status = json.loads(result.stdout)
    assert status["supports_vpnless"], "vpnless should be enabled by host"
