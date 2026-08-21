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


import json
from pathlib import Path

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_bxl_root(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl:root.bxl:root_test",
    )

    assert str(bsmr.cwd) in result.stdout


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_bxl_cell_root(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "upstream//cell_root.bxl:cell_root_test",
    )

    assert str(bsmr.cwd / "fbcode") in result.stdout


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_bxl_instant_event(bsmr: Bsmr) -> None:
    await bsmr.bxl(
        "//bxl/event.bxl:good",
    )

    # Get event log
    log = (await bsmr.log("show")).stdout.strip()
    lines = log.splitlines()
    # try to find starlark instant event

    found_event = False
    for line in lines:
        if "StarlarkUser" in line:
            assert "foo" in line
            assert "bool_value" in line
            assert "string_value" in line
            assert "int_value" in line
            found_event = True
            break

    if not found_event:
        raise AssertionError("Failed to find starlark instant event.")

    # Shouldn't fail
    await bsmr.bxl("//bxl/event.bxl:metadata_with_duration")

    await expect_failure(
        bsmr.bxl(
            "//bxl/event.bxl:bad_metadata",
        ),
        stderr_regex="Metadata should be a dict where keys are strings, and values are strings, ints, bools, or dicts/lists of the mentioned types. Got type: `list`",
    )

    await expect_failure(
        bsmr.bxl(
            "//bxl/event.bxl:bad_metadata_key",
        ),
        stderr_regex="Metadata keys should be strings. Got type: `int`",
    )

    await expect_failure(
        bsmr.bxl(
            "//bxl/event.bxl:bad_metadata_value",
        ),
        stderr_regex="Metadata values should be strings, ints, bools, or dicts/lists of the mentioned types. Key `key` had value type `tuple`",
    )

    result = await bsmr.bxl(
        "//bxl/event.bxl:ensured_artifact",
    )

    artifact_path = result.stdout.strip()

    # Get event log
    lines = (await bsmr.log("show-user")).stdout.strip().splitlines()
    found_event = False
    for line in lines:
        if "StarlarkUserEvent" in line:
            metadata = json.loads(line)["StarlarkUserEvent"]["metadata"]
            assert metadata["rel_path"] == artifact_path
            assert metadata["abs_path"] == str(Path(bsmr.cwd / artifact_path))
            assert metadata["nested"]["nested_artifact"] == artifact_path
            found_event = True
            break

    if not found_event:
        raise AssertionError("Failed to find starlark instant event.")


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_bxl_read_config(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "-c",
        "key.section=foo",
        "//bxl/read_config.bxl:read_config_test",
    )

    assert "foo" in result.stdout
    assert "True" in result.stdout


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_load_file(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl:load_file.bxl:load_test",
    )

    assert str(bsmr.cwd) in result.stdout
