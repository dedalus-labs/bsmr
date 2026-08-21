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

# This file acts as both a test of `bsmr.allow_eden_io` as well as a self-test
# of the `setup_eden` logic in the test runner


async def _check_io_provider(bsmr: Bsmr, name: str) -> None:
    await bsmr.server()
    out = await bsmr.status()
    status = json.loads(out.stdout.strip())
    io_provider = status["io_provider"]
    assert io_provider == name


@bsmr_test(
    setup_eden=False,
    extra_bsmr_config={
        "bsmr": {
            "allow_eden_io": "false",
        }
    },
)
async def test_no_eden(bsmr: Bsmr) -> None:
    await _check_io_provider(bsmr, "fs")


@bsmr_test(
    setup_eden=False,
    extra_bsmr_config={
        "bsmr": {
            "allow_eden_io": "true",
        }
    },
)
async def test_allow_eden_io_ignored_on_fs_io(bsmr: Bsmr) -> None:
    await _check_io_provider(bsmr, "fs")


@bsmr_test(
    setup_eden=True,
    extra_bsmr_config={
        "bsmr": {
            "allow_eden_io": "false",
        }
    },
)
async def test_allow_eden_io_respected(bsmr: Bsmr) -> None:
    await _check_io_provider(bsmr, "fs")


@bsmr_test(
    setup_eden=True,
    extra_bsmr_config={
        "bsmr": {
            "allow_eden_io": "true",
        }
    },
)
async def test_eden_io(bsmr: Bsmr) -> None:
    await _check_io_provider(bsmr, "eden")
