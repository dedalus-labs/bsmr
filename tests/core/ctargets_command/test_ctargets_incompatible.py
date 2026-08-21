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


import re

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


def _replace_hash(s: str) -> str:
    return re.sub(r"\b[0-9a-f]{16}\b", "<HASH>", s)


@bsmr_test()
async def test_ctargets_incompatible(bsmr: Bsmr) -> None:
    result = await bsmr.ctargets(
        # This one will be omitted from the output because it is not compatible.
        "root//:triangle",
        # This one will be output.
        "root//:square",
        "--target-platforms=root//:rectangular",
    )
    stdout = _replace_hash(result.stdout)
    [line] = stdout.splitlines()
    assert line == "root//:square (root//:rectangular#<HASH>)"

    assert "Skipped 1 incompatible targets" in result.stderr
    assert "root//:triangle (root//:rectangular#" in result.stderr
