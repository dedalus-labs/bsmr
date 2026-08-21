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


@bsmr_test()
async def test_ctargets_basic(bsmr: Bsmr) -> None:
    result = await bsmr.ctargets(
        "root//:gum",
        "--target-platforms=root//:p",
    )
    [line] = result.stdout.splitlines()
    line = _replace_hash(line)
    assert line == "root//:gum (root//:p#<HASH>)"


@bsmr_test()
async def test_ctargets_json(bsmr: Bsmr) -> None:
    result = await bsmr.ctargets(
        "root//:chocolate",
        "--json",
    )

    [output] = json.loads(result.stdout)

    output["bsmr.type"]
    output["bsmr.deps"]
    output["bsmr.inputs"]
    output["bsmr.package"]
    output["name"]
    assert output["default_target_platform"] == "root//:p"
    output["visibility"]
    output["within_view"]


@bsmr_test()
async def test_ctargets_multi_json(bsmr: Bsmr) -> None:
    result = await bsmr.ctargets(
        "root//:",
        "--json",
    )

    outputs = json.loads(result.stdout)

    assert len(outputs) == 3

    for output in outputs:
        output["bsmr.type"]
        output["bsmr.deps"]
        output["bsmr.inputs"]
        output["bsmr.package"]

        name = output["name"]
        if name == "chocolate":
            assert output["default_target_platform"] == "root//:p"

        output["visibility"]
        output["within_view"]


@bsmr_test()
async def test_ctargets_output_attribute(bsmr: Bsmr) -> None:
    result = await bsmr.ctargets(
        "root//:chocolate", "--output-attribute=default_*", "--output-attribute=name"
    )

    [output] = json.loads(result.stdout)

    assert {
        "name": "chocolate",
        "default_target_platform": "root//:p",
    } == output
