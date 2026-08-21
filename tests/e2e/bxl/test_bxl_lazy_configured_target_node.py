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
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_lazy_configured_target(bsmr: Bsmr) -> None:
    await bsmr.bxl(
        "//bxl/lazy_configured_target_node.bxl:lazy_configured_target_node_resolve",
    )


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_lazy_configured_target_error(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.bxl(
            "//bxl/lazy_configured_target_node.bxl:lazy_configured_target_node_resolve_error"
        ),
        stderr_regex="root//incompatible_targets:incompatible",
    )


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_lazy_configured_target_catch_error(bsmr: Bsmr) -> None:
    await bsmr.bxl(
        "//bxl/lazy_configured_target_node.bxl:lazy_configured_target_node_resolve_catch_error",
    )


@bsmr_test(inplace=False, data_dir="bxl/simple")
async def test_lazy_configured_target_node_pattern(bsmr: Bsmr) -> None:
    await bsmr.bxl(
        "//bxl/lazy_configured_target_node.bxl:lazy_configured_target_node_pattern",
    )
