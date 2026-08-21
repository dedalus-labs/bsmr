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
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


def _replace_hash(s: str) -> str:
    return re.sub(r"\b[0-9a-f]{16}\b", "<HASH>", s)


@bsmr_test()
async def test_ctargets_skip_missing_targets(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.ctargets(
            "root//:existing",
            "root//:nonexistent",
            "--target-platforms=root//:p",
        ),
        stderr_regex="Unknown target `nonexistent` from package",
    )

    result = await bsmr.ctargets(
        "root//:existing",
        "root//:nonexistent",
        "--target-platforms=root//:p",
        "--skip-missing-targets",
    )
    [line] = result.stdout.splitlines()
    line = _replace_hash(line)
    assert line == "root//:existing (root//:p#<HASH>)"

    assert "Skipped 1 missing targets:" in result.stderr
    assert "root//:nonexistent" in result.stderr
