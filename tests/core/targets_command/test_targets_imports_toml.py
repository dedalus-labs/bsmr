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
async def test_imports_toml(bsmr: Bsmr) -> None:
    """Test that targets --streaming --imports handles TOML file imports."""
    result = await bsmr.targets("//...", "--json", "--streaming", "--imports")
    xs = json.loads(result.stdout)

    found_targets = False
    found_bzl = False
    found_toml = False

    for x in xs:
        if "bsmr.imports" not in x:
            continue
        file = x["bsmr.file"]
        imports = x["bsmr.imports"]

        if file == "root//TARGETS.fixture":
            assert "root//uses_toml.bzl" in imports
            found_targets = True
        elif file == "root//uses_toml.bzl":
            assert "root//data.toml" in imports
            found_bzl = True
        elif file == "root//data.toml":
            assert imports == []
            found_toml = True

    assert found_targets, "TARGETS.fixture imports should be reported"
    assert found_bzl, "uses_toml.bzl imports (including data.toml) should be reported"
    assert found_toml, "data.toml should appear as an import with empty sub-imports"
