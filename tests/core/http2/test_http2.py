# ===----------------------------------------------------------------------===
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

from bsmr.tests.e2e_util.api.buck import Buck
from bsmr.tests.e2e_util.buck_workspace import buck_test


@buck_test()
async def test_http2_enabled(buck: Buck) -> None:
    # Get a daemon to start
    await buck.build()
    result = await buck.status()
    status = json.loads(result.stdout)
    assert status["http2"] is True, "http2 is enabled by default"

    # Insert necessary bsmrconfig to pick up http2 configuration.
    with open(f"{buck.cwd}/.bsmr", "a") as bsmrconfig:
        bsmrconfig.writelines(["[http]\n", "http2 = false\n"])

    # Get a daemon to start
    await buck.build()
    result = await buck.status()
    status = json.loads(result.stdout)
    assert status["http2"] is False, "http2 was disabled by bsmrconfig"
