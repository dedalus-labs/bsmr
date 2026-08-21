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


import os
import re
from pathlib import Path

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.api.bsmr_result import BsmrResult
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


def _includes(output: BsmrResult) -> list[str]:
    return sorted(
        [
            re.sub(".*[/\\\\]", "", line)
            for line in output.stdout.splitlines()
            if line.endswith(".bzl") or line.endswith(".json")
        ]
    )


@bsmr_test()
async def test_audit_includes(bsmr: Bsmr, tmp_path: Path) -> None:
    expected_includes = ["example.json", "incl.bzl", "prelude.bzl"]
    # Using project relative path.
    output = await bsmr.audit("includes", "TARGETS.fixture")
    assert _includes(output) == expected_includes

    # Using project relative path when in a subdirectory.
    await bsmr.audit("includes", "TARGETS.fixture", rel_cwd=Path("dir"))
    assert _includes(output) == expected_includes

    # Using absolute path.
    output = await bsmr.audit("includes", f"{bsmr.cwd}/TARGETS.fixture")
    assert _includes(output) == expected_includes

    if os.name != "nt":
        # Create symlink to the project root in a temporary directory.
        (tmp_path / "symlink").symlink_to(bsmr.cwd)

        output = await bsmr.audit("includes", f"{tmp_path}/symlink/TARGETS.fixture")
        assert _includes(output) == expected_includes
