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
async def test_specific_target_success(bsmr: Bsmr) -> None:
    """Test unconfigured_targets_keep_going with a specific successful target."""
    await bsmr.bxl(
        "//:unconfigured_targets_keep_going.bxl:test_specific_target_success",
    )


@bsmr_test()
async def test_recursive_pattern_success(bsmr: Bsmr) -> None:
    """Test unconfigured_targets_keep_going with a recursive pattern that includes only successful packages."""
    await bsmr.bxl(
        "//:unconfigured_targets_keep_going.bxl:test_recursive_pattern_success",
    )


@bsmr_test()
async def test_recursive_pattern_mixed(bsmr: Bsmr) -> None:
    """Test unconfigured_targets_keep_going with a recursive pattern that includes both successful and failing packages."""
    await bsmr.bxl(
        "//:unconfigured_targets_keep_going.bxl:test_recursive_pattern_mixed",
    )


@bsmr_test()
async def test_failing_package_only(bsmr: Bsmr) -> None:
    """Test unconfigured_targets_keep_going with a pattern that only matches a failing package."""
    await bsmr.bxl(
        "//:unconfigured_targets_keep_going.bxl:test_failing_package_only",
    )


@bsmr_test()
async def test_specific_target_in_failing_package(bsmr: Bsmr) -> None:
    """Test unconfigured_targets_keep_going with a specific target in a failing package."""
    await bsmr.bxl(
        "//:unconfigured_targets_keep_going.bxl:test_specific_target_in_failing_package",
    )
