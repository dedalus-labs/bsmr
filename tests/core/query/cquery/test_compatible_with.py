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


@bsmr_test()
async def test_compatible_with(bsmr: Bsmr) -> None:
    for good in ["root//:pass", "root//:pass2"]:
        out = await bsmr.cquery(good)
        assert re.match(
            "{} \\(.*\\)\n".format(good),
            out.stdout,
        )

    for bad in ["root//:fail", "root//:fail2"]:
        out = await bsmr.cquery(bad)
        assert out.stdout == ""
