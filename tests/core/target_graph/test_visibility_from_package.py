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


from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test
from bsmr.tests.e2e_util.helper.golden import golden


@bsmr_test()
async def test_visibility_from_package_simple(bsmr: Bsmr) -> None:
    result = await bsmr.uquery(
        "root//simple:", "--output-attribute=visibility|within_view"
    )
    golden(
        output=result.stdout,
        rel_path="simple/golden.uquery.json",
    )


@bsmr_test()
async def test_visibility_from_package_inherit(bsmr: Bsmr) -> None:
    result = await bsmr.uquery(
        "root//inherit/...", "--output-attribute=visibility|within_view"
    )
    golden(
        output=result.stdout,
        rel_path="inherit/golden.uquery.json",
    )


@bsmr_test()
async def test_visibility_from_package_override(bsmr: Bsmr) -> None:
    result = await bsmr.uquery(
        "root//override/...", "--output-attribute=visibility|within_view"
    )
    golden(
        output=result.stdout,
        rel_path="override/golden.uquery.json",
    )


@bsmr_test()
async def test_visibility_from_package_public(bsmr: Bsmr) -> None:
    result = await bsmr.uquery(
        "root//public/...", "--output-attribute=visibility|within_view"
    )
    golden(
        output=result.stdout,
        rel_path="public/golden.uquery.json",
    )
