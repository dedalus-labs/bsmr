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


import json

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


@bsmr_test()
async def test_daemon_buster(bsmr: Bsmr) -> None:
    async def pid() -> int:
        return json.loads((await bsmr.status()).stdout)["process_info"]["pid"]

    await bsmr.build(":")
    pid0 = await pid()

    await bsmr.build(":")
    pid1 = await pid()
    assert pid1 == pid0

    with open(bsmr.cwd / ".bsmr", "a") as f:
        f.write("[bsmr]\n")
        f.write("daemon_buster = 1\n")

    await bsmr.build(":")
    pid2 = await pid()
    assert pid2 != pid1

    await bsmr.build(":")
    pid3 = await pid()
    assert pid3 == pid2

    with open(bsmr.cwd / ".bsmr", "a") as f:
        f.write("[bsmr]\n")
        f.write("daemon_buster = 2\n")

    await bsmr.build(":")
    pid4 = await pid()
    assert pid4 != pid3

    with open(bsmr.cwd / ".bsmr", "r") as f:
        config = f.read()

    with open(bsmr.cwd / ".bsmr", "w") as f:
        f.write(
            "\n".join(
                line for line in config.splitlines() if "daemon_buster" not in line
            )
        )

    await bsmr.build(":")
    pid5 = await pid()
    assert pid5 != pid4
