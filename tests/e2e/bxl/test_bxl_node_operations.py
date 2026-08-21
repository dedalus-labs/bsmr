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
import re

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


def _replace_hash(s: str) -> str:
    return re.sub(r"\b[0-9a-f]{16}\b", "<HASH>", s)


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_bxl_label_functions(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl/label_functions.bxl:label_func_test",
    )

    assert _replace_hash(result.stdout).splitlines() == [
        "root//bin:the_binary (root//platforms:platform1#<HASH>)",
        "root//bin:the_binary[sub] (root//platforms:platform1#<HASH>)",
        # configured_target() called for below, should only return configured target
        "root//bin:the_binary (root//platforms:platform1#<HASH>)",
        "root//bin:the_binary[sub1][sub2] (root//platforms:platform1#<HASH>)",
        # configured_target() called for below, should only return configured target
        "root//bin:the_binary (root//platforms:platform1#<HASH>)",
        "root//bin:the_binary",
        "root//bin:the_binary[sub]",
        "root//bin:the_binary[sub1][sub2]",
    ]


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_bxl_coerced_attrs(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl/coerced_attributes.bxl:coerced_attrs_test",
    )

    result = await bsmr.bxl(
        "//bxl/coerced_attributes.bxl:coerced_attributes_display_json_test",
    )
    cmd_select = json.loads(result.stdout)
    assert cmd_select["__type"] == "selector"
    assert cmd_select["entries"] == {
        "DEFAULT": "foo",
        "config//os:macos": "bar",
        "config//os:windows": "foobar",
    }

    result = await bsmr.bxl(
        "//bxl/coerced_attributes.bxl:coerced_attributes_display_test",
    )

    output = result.stdout

    assert "root//platforms:platform1" in output
    assert "genrule_with_selects" in output
    assert (
        'select({"config//os:macos": "bar", "config//os:windows": "foobar", "DEFAULT": "foo"})'
        in output
    )
    assert "PUBLIC" in output
    assert "magic" in output

    await bsmr.bxl(
        "//bxl/coerced_attributes.bxl:selector_attrs_test",
    )
    await bsmr.bxl(
        "//bxl/coerced_attributes.bxl:concat_attrs_test",
    )


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_bxl_configured_node(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl/node.bxl:configured_node_test",
    )

    assert _replace_hash(result.stdout).splitlines() == [
        "root//bin:the_binary (root//platforms:platform1#<HASH>)",
        "root//rules/rules.bzl:_foo_binary",
        "normal",
    ]


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_bxl_unconfigured_node(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl/node.bxl:unconfigured_node_test",
    )

    assert result.stdout.splitlines() == [
        "root//bin:the_binary",
        "root//rules/rules.bzl:_foo_binary",
        "normal",
        "[root//bin/TARGETS.fixture]",
    ]


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_node_attrs(bsmr: Bsmr) -> None:
    await bsmr.bxl(
        "//bxl:node_attributes.bxl:attrs_test",
    )


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_lazy_node_attrs(bsmr: Bsmr) -> None:
    await bsmr.bxl(
        "//bxl:node_attributes.bxl:lazy_attrs_test",
    )


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_lazy_node_attrs_with_special_attrs(bsmr: Bsmr) -> None:
    await bsmr.bxl(
        "//bxl:node_attributes.bxl:lazy_attrs_with_special_attrs_test",
    )


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_resolved_node_attrs(bsmr: Bsmr) -> None:
    await bsmr.bxl(
        "//bxl:resolved_node_attributes.bxl:resolved_attrs_test",
    )


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_lazy_resolved_node_attrs(bsmr: Bsmr) -> None:
    await bsmr.bxl(
        "//bxl:resolved_node_attributes.bxl:lazy_resolved_attrs_test",
    )


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_lazy_resolved_node_with_special_attrs(bsmr: Bsmr) -> None:
    await bsmr.bxl(
        "//bxl:resolved_node_attributes.bxl:lazy_resolved_attrs_with_special_attrs_test",
    )


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_unconfigured_target_node_attrs(bsmr: Bsmr) -> None:
    await bsmr.bxl(
        "//bxl:unconfigure_target_node_attrs.bxl:node_attrs",
    )

    result = await bsmr.bxl(
        "//bxl/unconfigure_target_node_attrs.bxl:node_attrs_display",
    )

    output = result.stdout

    assert "root//platforms:platform1" in output
    assert "genrule_with_selects" in output
    assert (
        'select({"config//os:macos": "bar", "config//os:windows": "foobar", "DEFAULT": "foo"})'
        in output
    )
    assert "PUBLIC" in output
    assert "magic" in output

    await bsmr.bxl(
        "//bxl/unconfigure_target_node_attrs.bxl:selector_attrs_test",
    )
    await bsmr.bxl(
        "//bxl/unconfigure_target_node_attrs.bxl:concat_attrs_test",
    )

    await bsmr.bxl(
        "//bxl/unconfigure_target_node_attrs.bxl:attr_metadata",
    )


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_bxl_node_constraints(bsmr: Bsmr) -> None:
    await bsmr.bxl(
        "//bxl/node_constraints.bxl:constraints_test",
    )


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_bxl_node_empty_constraints(bsmr: Bsmr) -> None:
    await bsmr.bxl(
        "//bxl/node_constraints.bxl:empty_constraints_test",
    )


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_configured_target_node_attrs(bsmr: Bsmr) -> None:
    await bsmr.bxl(
        "//bxl:configured_target_node_attrs.bxl:attrs_test",
    )


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_bxl_providers_info(bsmr: Bsmr) -> None:
    await bsmr.bxl(
        "//bxl/providers_info.bxl:providers_info_test",
    )
