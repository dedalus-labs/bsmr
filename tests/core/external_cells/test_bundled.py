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
from pathlib import Path

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


@bsmr_test()
async def test_bsmrconfig_works_in_external_cells(bsmr: Bsmr) -> None:
    result = await bsmr.audit(
        "config", "--cell", "test_bundled_cell", "user_section.key"
    )
    assert "key = value" in result.stdout


@bsmr_test()
async def test_uquery(bsmr: Bsmr) -> None:
    result = await bsmr.uquery("deps(other//:other_alias)")
    assert result.stdout.strip().split() == [
        "test_bundled_cell//dir:test_hidden",
        "test_bundled_cell//dir:test",
        "other//:other_alias",
    ]
    result = await bsmr.uquery(
        "deps(test_bundled_cell//dir:test)", rel_cwd=Path("other")
    )
    assert result.stdout.strip().split() == [
        "test_bundled_cell//dir:test_hidden",
        "test_bundled_cell//dir:test",
    ]


@bsmr_test()
async def test_build_local(bsmr: Bsmr) -> None:
    result = await bsmr.build_without_report(
        "--show-full-simple-output", "--local-only", "other//:other_alias"
    )
    p = Path(result.stdout.strip())
    assert p.read_text().strip() == "\n".join(["value", "6", "foobar", "foobar2"])


@bsmr_test()
async def test_build_remote(bsmr: Bsmr) -> None:
    result = await bsmr.build_without_report(
        "--show-full-simple-output", "--remote-only", "other//:other_alias"
    )
    p = Path(result.stdout.strip())
    assert p.read_text().strip() == "\n".join(["value", "6", "foobar", "foobar2"])


@bsmr_test()
async def test_materialize_source_directly(bsmr: Bsmr) -> None:
    result = await bsmr.build_without_report(
        "--show-full-simple-output", "test_bundled_cell//dir:exported"
    )
    p = Path(result.stdout.strip())
    assert f"external_cells{os.path.sep}bundled" in str(p)
    assert str(p).endswith("src.txt")
    assert p.read_text().strip() == "foobar"


@bsmr_test()
async def test_expand_external_cell(bsmr: Bsmr) -> None:
    await bsmr.expand_external_cell("test_bundled_cell")
    assert (bsmr.cwd / "test_bundled_cell" / ".bsmr").exists()

    # Remove the external cell declaration
    (bsmr.cwd / ".bsmr_no_external").replace(bsmr.cwd / ".bsmr")
    (bsmr.cwd / "test_bundled_cell" / "dir" / "src.txt").write_text("foobar3\n")

    result = await bsmr.build_without_report(
        "--show-full-simple-output", "other//:other_alias"
    )
    p = Path(result.stdout.strip())
    assert p.read_text().strip() == "\n".join(["value", "6", "foobar3", "foobar2"])
