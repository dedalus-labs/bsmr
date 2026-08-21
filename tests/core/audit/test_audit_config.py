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


@bsmr_test(
    extra_bsmr_config={
        "test": {
            "foo": "bar",
        }
    },
)
async def test_extra_bsmr_config(bsmr: Bsmr) -> None:
    """
    Assert that our testing framework works as expected.
    """

    cfg = (await bsmr.audit_config("--style=json")).get_json()
    assert cfg.get("test.foo") == "bar"


@bsmr_test()
async def test_audit_config_json(bsmr: Bsmr) -> None:
    result = await bsmr.audit_config("--style=json")
    result_json = result.get_json()
    assert result_json is not None


@bsmr_test()
async def test_audit_config_cell_json(bsmr: Bsmr) -> None:
    out = await bsmr.audit_config(
        "--style",
        "json",
    )
    out_json = out.get_json() or {}
    assert out_json.get("test.is_root") == "yes"
    assert out_json.get("test.is_code") is None

    out = await bsmr.audit_config("--style", "json", "--cell", "code")
    out_json = out.get_json() or {}
    assert out_json.get("test.is_code") == "yes"
    assert out_json.get("test.is_root") is None

    out = await bsmr.audit_config(
        "--style",
        "json",
        rel_cwd=Path("code"),
    )
    out_json = out.get_json() or {}
    assert out_json.get("test.is_code") == "yes"
    assert out_json.get("test.is_root") is None


@bsmr_test(setup_eden=True)
async def test_audit_config_all_cells(bsmr: Bsmr) -> None:
    out = await bsmr.audit_config(
        "--all-cells",
        "--style",
        "json",
    )
    out_json = out.get_json() or {}
    print(out_json)
    assert out_json.get("code//bar.a") == "2"
    assert out_json.get("source//bar.a") == "1"
    assert out_json.get("root//bar.a") == "1"
    assert out_json.get("b//bar.a") is None

    out = await bsmr.audit_config(
        "--all-cells",
        "--style",
        "json",
        "code//bar.a",
    )
    out_json = out.get_json() or {}
    assert out_json.get("code//bar.a") == "2"
    assert out_json.get("source//bar.a") is None

    out = await bsmr.audit_config(
        "--all-cells",
    )
    assert "# Cell: source\n[bar]\n    a = 1\n" in out.stdout


@bsmr_test()
async def test_audit_config_with_config_value(bsmr: Bsmr) -> None:
    result_config = await bsmr.audit_config(
        "python",
        "--style",
        "json",
        "-cpython.helpers=true",
    )
    result_config_json = result_config.get_json()

    assert result_config_json.get("python.helpers") == "true"


@bsmr_test()
async def test_audit_config_with_config_file(bsmr: Bsmr, tmp_path: Path) -> None:
    configfile = tmp_path / "config.bcfg"
    configfile.write_text("[python]\n  helpers = true\n")

    result_file = await bsmr.audit_config(
        "--config-file",
        str(configfile),
        "--style",
        "json",
    )

    assert result_file.get_json().get("python.helpers") == "true"


@bsmr_test()
async def test_audit_config_location_extended(bsmr: Bsmr) -> None:
    result = await bsmr.audit_config(
        "bar.a",
        "--location=extended",
    )
    assert "a = 1" in result.stdout
    assert "included.bcfg:2" in result.stdout


@bsmr_test()
async def test_audit_config_with_cell_syntax(bsmr: Bsmr) -> None:
    result_file = await bsmr.audit_config(
        "code//test.is_code",
        "--style",
        "json",
    )
    result_file_json = result_file.get_json()

    assert result_file_json.get("code//test.is_code") == "yes"


@bsmr_test()
async def test_cell_relative_configs(bsmr: Bsmr) -> None:
    result_root_cell = await bsmr.audit_config(
        "--config",
        "root//bar.a=5",
        "--style",
        "json",
    )
    result_root_cell_json = result_root_cell.get_json()

    assert result_root_cell_json is not None
    assert result_root_cell_json.get("foo.b") == "5"

    result_nonroot_cell = await bsmr.audit_config(
        "foo",
        "--config",
        "code//bar.a=5",
        "--style",
        "json",
        "--cell",
        "code",
    )
    result_nonroot_cell_json = result_nonroot_cell.get_json()

    assert result_nonroot_cell_json is not None
    assert result_nonroot_cell_json.get("foo.b") == "5"

    result_diff_cell = await bsmr.audit_config(
        "foo",
        "--config",
        "code//bar.a=5",
        "--style",
        "json",
        "--cell",
        "source",
    )
    result_diff_cell_json = result_diff_cell.get_json()

    assert result_diff_cell_json is not None
    assert result_diff_cell_json.get("foo.b") == "1"

    result_all_cell = await bsmr.audit_config(
        "foo",
        "--config",
        "bar.a=5",
        "--style",
        "json",
        "--cell",
        "source",
    )
    result_all_cell_json = result_all_cell.get_json()

    assert result_all_cell_json is not None
    assert result_all_cell_json.get("foo.b") == "5"
