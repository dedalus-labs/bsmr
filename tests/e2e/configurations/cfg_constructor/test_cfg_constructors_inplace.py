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
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test

# Test `cfg_constructors` end to end. This is useful for testing the core + starlark
# implementation so that we know we don't break anything in the repo. For testing
# specific cfg constructor logic, use bxl_test to unit test the cfg constructor instead


@bsmr_test(inplace=True)
async def test_cfg_constructor_without_modifiers_returns_same_configuration(
    bsmr: Bsmr,
) -> None:
    result = await bsmr.cquery(
        "root//tests/e2e/configurations/cfg_constructor/test_clear_package_modifiers_data/test_cfg_constructor_data:no_modifiers",
        "-A",
    )
    result = json.loads(result.stdout)
    assert len(result) == 1
    _test_target, test_target_attrs = list(result.items())[0]
    assert test_target_attrs["bsmr.target_configuration"].startswith(
        "ovr_config//platform:base"
    )


@bsmr_test(inplace=True)
async def test_cfg_constructor_with_target_modifiers(bsmr: Bsmr) -> None:
    result = await bsmr.cquery(
        "root//tests/e2e/configurations/cfg_constructor/test_clear_package_modifiers_data/test_cfg_constructor_data:has_target_modifier",
        "-A",
    )
    result = json.loads(result.stdout)
    assert len(result) == 1
    _test_target, test_target_attrs = list(result.items())[0]
    assert test_target_attrs["bsmr.target_configuration"].startswith("cfg:linux")


@bsmr_test(
    inplace=True,
    extra_bsmr_config={
        # CLI modifier validation is disabled for users and enabled for CI. To make sure this test case always has CLI modifier validation enabled,
        # explicitly enable it here.
        "bsmr": {"skip_cli_modifier_validation_DO_NOT_SET_TO_TRUE_ON_CI": ""}
    },
)
async def test_invoke_cfg_constructors_with_cli_modifier_validation(bsmr: Bsmr) -> None:
    await bsmr.cquery(
        "root//tests/e2e/configurations/cfg_constructor/test_clear_package_modifiers_data/test_cfg_constructor_data:has_target_modifier",
        "--modifier=ovr_config//os:linux",
    )
    await expect_failure(
        bsmr.cquery(
            "root//tests/e2e/configurations/cfg_constructor/test_clear_package_modifiers_data/test_cfg_constructor_data:has_target_modifier",
            "--modifier=root//tests/e2e/configurations/cfg_constructor/test_clear_package_modifiers_data/test_cfg_constructor_data:some_constraint_value",
        ),
        stderr_regex="Only a select number of modifiers are allowed to be set from CLI on CI",
    )
