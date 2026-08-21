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
from bsmr.tests.e2e_util.helper.utils import json_get


@bsmr_test()
async def test_no_dice_invalidation_on_root_directory_changes(bsmr: Bsmr) -> None:
    await bsmr.build("root//dir:")

    # Add a file to the root directory
    (bsmr.cwd / "file.txt").write_text("hello world")

    await bsmr.build("root//dir:")

    log = (await bsmr.log("show")).stdout.splitlines()

    for line in log:
        e = json_get(
            line,
            "Event",
            "data",
            "SpanEnd",
            "data",
            "Load",
        )
        assert e is None, "Should not have loaded anything"
