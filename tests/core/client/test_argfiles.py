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

from pathlib import Path

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


@bsmr_test()
async def test_argfile_with_cell(bsmr: Bsmr) -> None:
    res = await bsmr.audit_config("@cell1//argfile", "--cell", "root", "foo.bar")
    assert "bar = 1" in res.stdout


@bsmr_test()
async def test_argfile_from_cwd_cell(bsmr: Bsmr) -> None:
    res = await bsmr.audit_config(
        "@//argfile",
        "--cell",
        "root",
        "foo.bar",
        rel_cwd=Path("cell1"),
    )
    assert "bar = 1" in res.stdout


@bsmr_test()
async def test_executable_argfile(bsmr: Bsmr) -> None:
    res = await bsmr.audit_config(
        "@//exec_argfile.py#iphonesimulator-x86_64", "--cell", "root", "foo.bar"
    )
    assert "bar = 1" in res.stdout


@bsmr_test()
async def test_stdin_argfile(bsmr: Bsmr) -> None:
    res = await bsmr.audit_config(
        "@-",
        "--cell",
        "root",
        "foo.bar",
        input=str.encode("--config=foo.bar=1"),
    )
    assert "bar = 1" in res.stdout
