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
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test
from bsmr.tests.e2e_util.helper.golden import golden


@bsmr_test()
async def test_target_call_stacks_default(bsmr: Bsmr) -> None:
    result = await bsmr.uquery(
        "--stack",
        "root//:test",
    )
    golden(
        output=result.stdout,
        rel_path="golden/uquery.stdout",
    )
    result = await bsmr.cquery(
        "--stack",
        "root//:test",
    )
    golden(
        output=result.stdout,
        rel_path="golden/cquery.stdout",
    )
