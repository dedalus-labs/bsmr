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


@bsmr_test()
async def test_get_package_path(bsmr: Bsmr) -> None:
    await bsmr.bxl(
        "//package.bxl:get_package_path",
    )


@bsmr_test()
async def test_read_package_value(bsmr: Bsmr) -> None:
    await bsmr.bxl("//package.bxl:read_package_value")


@bsmr_test()
async def test_read_package_value_from_string(bsmr: Bsmr) -> None:
    await bsmr.bxl("//package.bxl:read_package_value_from_string")


@bsmr_test()
async def test_read_override_package_value(bsmr: Bsmr) -> None:
    await bsmr.bxl("//package.bxl:read_override_package_value")


@bsmr_test()
async def test_read_package_value_not_found(bsmr: Bsmr) -> None:
    await bsmr.bxl("//package.bxl:read_package_value_not_found")


@bsmr_test()
async def test_read_package_visibility(bsmr: Bsmr) -> None:
    await bsmr.bxl("//package.bxl:read_package_visibility")


@bsmr_test()
async def test_read_package_within_view(bsmr: Bsmr) -> None:
    await bsmr.bxl("//package.bxl:read_package_within_view")


@bsmr_test()
async def test_read_package_visibility_cap(bsmr: Bsmr) -> None:
    await bsmr.bxl("//package.bxl:read_package_visibility_cap")
