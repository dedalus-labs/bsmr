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
async def test_imports(bsmr: Bsmr) -> None:
    result = await bsmr.targets("//...", "--json", "--streaming", "--imports")
    xs = json.loads(result.stdout)
    found = 0
    for x in xs:
        if "bsmr.imports" in x:
            if x["bsmr.file"] == "root//TARGETS.fixture":
                assert x["bsmr.package"] == "root//"
                assert x["bsmr.imports"] == ["prelude//prelude.bzl", "root//a.bzl"]
                found += 1
            elif x["bsmr.file"] == "root//a.bzl":
                assert x["bsmr.imports"] == [
                    "prelude//prelude.bzl",
                    "root//b.bzl",
                ]
                assert "bsmr.package" not in x
                found += 1
            elif x["bsmr.file"] == "root//PACKAGE":
                assert x["bsmr.imports"] == [
                    "prelude//prelude.bzl",
                    "root//b.bzl",
                ]
                assert "bsmr.package" not in x
                found += 1
    assert found == 3
