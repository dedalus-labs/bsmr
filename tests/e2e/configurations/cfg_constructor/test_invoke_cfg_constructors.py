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


@bsmr_test(inplace=False)
async def test_invoke_cfg_constructors(bsmr: Bsmr) -> None:
    result = await bsmr.cquery("root//:test")
    assert "root//:test (post_constraint_analysis_test_label" in result.stdout


@bsmr_test(inplace=False)
async def test_invoke_cfg_constructors_without_aliases(bsmr: Bsmr) -> None:
    # This test ensures that for backwards compatibility, we can call
    # `set_cfg_constructor` without explicitly passing in aliases parameter.
    result = await bsmr.cquery("root//:test", "-c", "testing.no_aliases=true")
    assert "root//:test (post_constraint_analysis_test_label" in result.stdout


@bsmr_test(inplace=False)
async def test_invoke_cfg_constructors_unbound_platform(bsmr: Bsmr) -> None:
    result = await bsmr.cquery("root//:test_unbound")
    assert "root//:test_unbound (post_constraint_analysis_test_label" in result.stdout
