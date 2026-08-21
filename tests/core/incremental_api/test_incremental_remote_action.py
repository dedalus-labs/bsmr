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


from __future__ import annotations

import json

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test
from bsmr.tests.e2e_util.helper.utils import random_string


@bsmr_test()
async def test_incremental_file_materialized(bsmr: Bsmr) -> None:
    result = await bsmr.run("root//:plate", "-c", f"test.seed={random_string()}")
    assert result.stdout == "0"
    result = await bsmr.run("root//:plate", "-c", f"test.seed={random_string()}")
    assert result.stdout == "1"


@bsmr_test()
async def test_incremental_dir_materialized(bsmr: Bsmr) -> None:
    result = await bsmr.run("root//:mate", "-c", f"test.seed={random_string()}")
    assert result.stdout == "0"
    result = await bsmr.run("root//:mate", "-c", f"test.seed={random_string()}")
    assert result.stdout == "1"


@bsmr_test()
async def test_incremental_file_not_materialized(bsmr: Bsmr) -> None:
    result = await bsmr.run("root//:flute", "-c", f"test.seed={random_string()}")
    assert result.stdout == "0"
    result = await bsmr.run("root//:flute", "-c", f"test.seed={random_string()}")
    assert result.stdout == "1"


@bsmr_test()
async def test_incremental_dir_not_materialized(bsmr: Bsmr) -> None:
    result = await bsmr.run("root//:suite", "-c", f"test.seed={random_string()}")
    assert result.stdout == "0"
    result = await bsmr.run("root//:suite", "-c", f"test.seed={random_string()}")
    assert result.stdout == "1"


@bsmr_test()
async def test_remote_cache_is_used(bsmr: Bsmr) -> None:
    seed = random_string()
    result = await bsmr.run("root//:plate", "-c", f"test.seed={seed}")
    assert result.stdout == "0"
    result = await bsmr.run("root//:plate", "-c", f"test.seed={random_string()}")
    assert result.stdout == "1"

    # For the next build with already used seed we expect the action to be taken from the cache
    result = await bsmr.run("root//:plate", "-c", f"test.seed={seed}")
    assert result.stdout == "0"

    out = await bsmr.log("what-ran", "--format", "json")
    out = [line.strip() for line in out.stdout.splitlines()]
    out = [json.loads(line) for line in out if line]
    assert len(out) == 1, "out should have 1 line: `{}`".format(out)
    repro = out[0]
    assert repro["reproducer"]["executor"] == "Cache"
