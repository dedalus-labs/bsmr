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

from bsmr.tests.e2e_util.api.buck import Buck
from bsmr.tests.e2e_util.buck_workspace import buck_test


@buck_test()
async def test_imports(buck: Buck) -> None:
    result = await buck.targets("//...", "--json", "--streaming", "--imports")
    xs = json.loads(result.stdout)
    found = 0
    for x in xs:
        if "buck.imports" in x:
            if x["buck.file"] == "root//TARGETS.fixture":
                assert x["buck.package"] == "root//"
                assert x["buck.imports"] == ["prelude//prelude.bzl", "root//a.bzl"]
                found += 1
            elif x["buck.file"] == "root//a.bzl":
                assert x["buck.imports"] == [
                    "prelude//prelude.bzl",
                    "root//b.bzl",
                ]
                assert "buck.package" not in x
                found += 1
            elif x["buck.file"] == "root//PACKAGE":
                assert x["buck.imports"] == [
                    "prelude//prelude.bzl",
                    "root//b.bzl",
                ]
                assert "buck.package" not in x
                found += 1
    assert found == 3
