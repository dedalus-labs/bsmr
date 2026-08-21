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


# Test select works with bsmrconfig.
@bsmr_test()
async def test_select_bsmrconfig(bsmr: Bsmr) -> None:
    out = await bsmr.cquery(
        "root//:the-test",
        "--output-attribute=labels",
    )
    q = json.loads(out.stdout)
    assert len(q) == 1
    assert list(q.values())[0]["labels"] == ["NO"]

    out = await bsmr.cquery(
        "root//:the-test",
        "--output-attribute=labels",
        "-c",
        "aaa.bbb=ccc",
    )
    q = json.loads(out.stdout)
    assert len(q) == 1
    assert list(q.values())[0]["labels"] == ["YES"]
