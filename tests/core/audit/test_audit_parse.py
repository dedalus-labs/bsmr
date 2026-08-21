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

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


@bsmr_test()
async def test_audit_parse(bsmr: Bsmr) -> None:
    # random config hash
    config_hash = "3f794b0267173c8e"

    # rule
    # json
    result = await bsmr.audit(
        "parse",
        f"bsmr-out/default/art/root/{config_hash}/path/to/target/__target_name__/output",
        "--json",
    )

    result = json.loads(result.stdout)
    assert result["cell_path"] == "root//path/to/target"
    assert result["target_label"] == "root//path/to/target:target_name"
    assert result["short_artifact_path"] == "output"
    assert result["config_hash"] == config_hash
    assert "content_hash" not in result
    assert (
        result["full_artifact_path_no_hash"]
        == "root/path/to/target/__target_name__/output"
    )

    # not json
    result = await bsmr.audit(
        "parse",
        f"bsmr-out/default/art/root/{config_hash}/path/to/target/__target_name__/output",
    )

    result = result.stdout.splitlines()
    assert result[0] == "root//path/to/target"
    assert result[1] == "root//path/to/target:target_name"
    assert result[2] == "output"
    assert result[3] == config_hash
    assert result[4] == "root/path/to/target/__target_name__/output"

    # output attribute
    result = await bsmr.audit(
        "parse",
        f"bsmr-out/default/art/root/{config_hash}/path/to/target/__target_name__/output",
        "--output-attribute",
        "config_hash",
        "--output-attribute",
        "full_artifact_path_no_hash",
    )

    result = result.stdout.splitlines()
    assert result[0] == config_hash
    assert result[1] == "root/path/to/target/__target_name__/output"

    # output attribute with json
    result = await bsmr.audit(
        "parse",
        f"bsmr-out/default/art/root/{config_hash}/path/to/target/__target_name__/output",
        "--json",
        "--output-attribute",
        "config_hash",
        "--output-attribute",
        "full_artifact_path_no_hash",
    )

    result = json.loads(result.stdout)
    assert result["config_hash"] == config_hash
    assert (
        result["full_artifact_path_no_hash"]
        == "root/path/to/target/__target_name__/output"
    )

    # tmp
    result = await bsmr.audit(
        "parse",
        f"bsmr-out/default/tmp/root/{config_hash}/path/to/target/__target_name__/output",
        "--json",
    )

    result = json.loads(result.stdout)
    assert result["cell_path"] == "root//path/to/target"
    assert result["target_label"] == "root//path/to/target:target_name"
    assert result["config_hash"] == config_hash
    assert (
        result["full_artifact_path_no_hash"]
        == "root/path/to/target/__target_name__/output"
    )

    # bxl
    result = await bsmr.audit(
        "parse",
        f"bsmr-out/default/art-bxl/root/{config_hash}/path/to/function.bxl/__function_name__/output",
        "--json",
    )

    result = json.loads(result.stdout)
    assert result["bxl_function_label"] == "root//path/to/function.bxl:function_name"
    assert result["config_hash"] == config_hash
    assert (
        result["full_artifact_path_no_hash"]
        == "root/path/to/function.bxl/__function_name__/output"
    )

    # anon
    result = await bsmr.audit(
        "parse",
        f"bsmr-out/default/art-anon/root/{config_hash}/path/to/target/rule_hash/__target_name__/output",
        "--json",
    )

    result = json.loads(result.stdout)
    assert result["cell_path"] == "root//path/to/target"
    assert result["target_label"] == "root//path/to/target:target_name"
    assert result["config_hash"] == config_hash
    assert result["attr_hash"] == "rule_hash"
    assert (
        result["full_artifact_path_no_hash"]
        == "root/path/to/target/rule_hash/__target_name__/output"
    )

    # test
    result = await bsmr.audit(
        "parse",
        f"bsmr-out/default/test/root/{config_hash}/path/to/target/__target_name__/output",
        "--json",
    )

    result = json.loads(result.stdout)
    assert result["cell_path"] == "root//path/to/target"
    assert result["config_hash"] == config_hash
    assert (
        result["full_artifact_path_no_hash"]
        == "root/path/to/target/__target_name__/output"
    )


@bsmr_test()
async def test_audit_parse_content_based(bsmr: Bsmr) -> None:
    # random content hash
    content_hash = "aaaabbbbccccdddd"

    # json
    result = await bsmr.audit(
        "parse",
        f"bsmr-out/default/art/root/path/to/target/__target_name__/{content_hash}/output",
        "--json",
    )

    result = json.loads(result.stdout)
    assert result["cell_path"] == "root//path/to/target"
    assert result["target_label"] == "root//path/to/target:target_name"
    assert result["short_artifact_path"] == "output"
    assert result["content_hash"] == content_hash
    assert "config_hash" not in result
    assert (
        result["full_artifact_path_no_hash"]
        == "root/path/to/target/__target_name__/output"
    )

    # not json
    result = await bsmr.audit(
        "parse",
        f"bsmr-out/default/art/root/path/to/target/__target_name__/{content_hash}/output",
    )

    result = result.stdout.splitlines()
    assert result[0] == "root//path/to/target"
    assert result[1] == "root//path/to/target:target_name"
    assert result[2] == "output"
    assert result[3] == content_hash
    assert result[4] == "root/path/to/target/__target_name__/output"
