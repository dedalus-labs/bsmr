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


import tempfile

import pytest
from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


@bsmr_test()
@pytest.mark.parametrize(  # type: ignore
    "cmd",
    ["build", "targets", "cquery", "bxl", "uquery"],
)
async def test_write_uuid(bsmr: Bsmr, cmd: str) -> None:
    with tempfile.NamedTemporaryFile() as file:
        cmd_call = getattr(bsmr, cmd)
        await expect_failure(cmd_call("--write-build-id", file.name, "a"))

        assert len(file.read()) > 0


@bsmr_test()
@pytest.mark.parametrize(  # type: ignore
    "cmd",
    ["build", "targets", "cquery", "bxl", "uquery"],
)
async def test_ban_cell_override(bsmr: Bsmr, cmd: str) -> None:
    cmd_call = getattr(bsmr, cmd)
    await expect_failure(cmd_call("--config", "repositories.foo=bar", "a"))
    await expect_failure(cmd_call("--config", "cells.foo=bar", "a"))
