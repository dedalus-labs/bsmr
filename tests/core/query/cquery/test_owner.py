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


@bsmr_test(data_dir="deprecated_correct")
async def test_owner_without_universe_correct(bsmr: Bsmr) -> None:
    # TODO(nga): there should be a warning.
    result = await bsmr.cquery(
        "owner(bin.sh)",
    )
    assert "" == result.stdout
    assert (
        "Query has no target literals and `--target-universe` is not specified"
        in result.stderr
    )


@bsmr_test(data_dir="deprecated_correct")
async def test_owner_with_auto_universe_correct(bsmr: Bsmr) -> None:
    result = await bsmr.cquery(
        "deps(//:test) intersect owner(bin.sh)",
    )
    lines = result.stdout.splitlines()
    # Drop configuration.
    targets = [t.split()[0] for t in lines]
    assert ["root//:bin"] == targets
