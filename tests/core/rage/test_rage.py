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


import os
import tempfile

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


def opener(path: str, flags: int) -> int:
    # Make it executable by user
    return os.open(path, flags, 0o777)


def mock_cmd_unix(path: str) -> None:
    with open(path, "w", opener=opener) as fl:
        fl.write(
            """\
#! /bin/sh
echo "$@"
        """
        )


# No windows since mocking pastry command didn't work D41623200
@bsmr_test(skip_for_os=["windows", "darwin"])
async def test_rage(bsmr: Bsmr) -> None:
    # Build a trivial action
    await bsmr.build("//:simple")

    with tempfile.TemporaryDirectory() as tmpdirname:
        pastry_path = f"{tmpdirname}/pastry"
        hg_path = f"{tmpdirname}/hg"
        mock_cmd_unix(pastry_path)
        mock_cmd_unix(hg_path)

        # We want to find our executable first
        cmd_path = tmpdirname + os.pathsep + os.environ["PATH"]
        # Run rage aginst the most recent invocation.
        await bsmr.rage(input=b"0", env={"PATH": cmd_path})


@bsmr_test(skip_for_os=["darwin"])
async def test_rage_no_paste(bsmr: Bsmr) -> None:
    # Build a trivial action
    await bsmr.build("//:simple")
    # Run rage aginst the most recent invocation.
    await bsmr.rage("--no-paste", "--invocation-offset", "0")


@bsmr_test(skip_for_os=["darwin"])
async def test_rage_no_logs(bsmr: Bsmr) -> None:
    # Rage doesn't crash even with no invocation logs
    await bsmr.rage("--no-paste")


@bsmr_test()  # pytest blows up if there's zero mac tests in the file
async def test_nop(bsmr: Bsmr) -> None:
    pass
