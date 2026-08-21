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

import subprocess

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


@bsmr_test(setup_eden=True, write_invocation_record=True)
async def test_build_count_since_rebase(bsmr: Bsmr) -> None:
    # needed for mergebase to exist
    subprocess.run(["sl", "bookmark", "main"], cwd=bsmr.cwd, check=True)
    res = await bsmr.build(
        "//:test",
    )
    record = res.invocation_record()
    print(record["hg_revision"])
    assert record["min_attempted_build_count_since_rebase"] == 1
    assert record["min_build_count_since_rebase"] == 1

    res2 = await expect_failure(
        bsmr.build(
            "//:test",
            "-c test.fail=1",
        )
    )
    record = res2.invocation_record()
    print(record["hg_revision"])
    assert record["min_attempted_build_count_since_rebase"] == 2
    assert record["min_build_count_since_rebase"] == 1
