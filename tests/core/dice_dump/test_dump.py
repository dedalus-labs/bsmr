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


import gzip
import os.path
from pathlib import Path

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


@bsmr_test()
async def test_debug_legacy_dice_dump(bsmr: Bsmr, tmp_path: Path) -> None:
    file_path = tmp_path / "dump"

    await bsmr.uquery("//...")
    await bsmr.debug("dice-dump", "--path", str(file_path))

    assert os.path.exists(f"{file_path}/nodes.gz")
    assert os.path.exists(f"{file_path}/edges.gz")
    assert os.path.exists(f"{file_path}/nodes_currently_running.gz")

    nodes = gzip.open(f"{file_path}/nodes.gz", "r").read().decode()
    assert "BuildDataKey" in nodes
    assert "FileOpsKey" in nodes

    edges = gzip.open(f"{file_path}/edges.gz", "r").read().decode()
    print(edges)
    assert edges  # check not empty

    nodes_currently_running = (
        gzip.open(f"{file_path}/nodes_currently_running.gz", "r").read().decode()
    )
    print(nodes_currently_running)
    assert nodes_currently_running == ""
