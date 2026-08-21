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
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


@bsmr_test(data_dir="include_external")
async def test_include_external_file(bsmr: Bsmr) -> None:
    # Note that the repo is inside a tempdir
    (bsmr.cwd.parent / "extra").write_text("[abc]\ndef=x", encoding="utf-8")
    await expect_failure(
        bsmr.audit_config("--cell", "root"),
        stderr_regex="Improperly include directive path",
    )


@bsmr_test(data_dir="empty", skip_for_os=["windows"])
async def test_external_symlink_resolution(bsmr: Bsmr, tmp_path: Path) -> None:
    base = tmp_path / "base"
    (base / "b" / "bb").mkdir(parents=True)
    (base / "a").mkdir()
    (base / "a" / "aa").symlink_to("../b/bb")
    (base / "b" / "included").write_text("[sec]\nval = physical", encoding="utf-8")
    (base / "a" / "included").write_text("[sec]\nval = logical", encoding="utf-8")

    (base / "b" / "bb" / "config").write_text("<file:../included>", encoding="utf-8")

    config_via_symlink = base / "a" / "aa" / "config"

    res = await bsmr.audit_config(
        "--cell", "root", "--config-file", str(config_via_symlink)
    )
    assert "val = physical" in res.stdout


@bsmr_test(data_dir="empty")
async def test_changing_external_include(bsmr: Bsmr) -> None:
    extra = bsmr.cwd.parent / "extra"
    extra.write_text("[abc]\n  def = 1", encoding="utf-8")

    # Start the daemon and build once
    await bsmr.audit_config(
        "--all-cells", env={"BSMR_TEST_EXTRA_EXTERNAL_CONFIG": str(extra)}
    )

    # Change the file and build again
    extra.write_text("[abc]\n    def = 2", encoding="utf-8")

    res = await bsmr.audit_config("--cell", "root", "abc.def")
    assert "[abc]\n    def = 2" in res.stdout
    res = await bsmr.audit_config("--cell", "cell", "abc.def")
    assert "[abc]\n    def = 2" in res.stdout


@bsmr_test(data_dir="include_through_symlink")
async def test_external_symlink_source_file(bsmr: Bsmr) -> None:
    external_dir = bsmr.cwd.parent / "extra"
    external_dir.mkdir()
    (bsmr.cwd / "repo_dir").symlink_to(external_dir)

    await bsmr.audit_config("--cell", "root", "abc.def")
