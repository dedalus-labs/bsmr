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


from bsmr.tests.e2e.configurations.cfg_constructor.modifiers_util import get_cfg
from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test

DATA_DIR = (
    "root//tests/e2e/configurations/cfg_constructor/test_cli_modifiers_data"
)
TARGET = f"{DATA_DIR}:test_target"
CONSTRAINT_A = f"{DATA_DIR}:A_1"
CONSTRAINT_B = f"{DATA_DIR}:B_1"
BSMR_TEST_MARKER = bsmr_test(
    inplace=True,
    extra_bsmr_config={
        # CLI modifier validation is disabled for users and enabled for CI. To make sure this test case always has CLI modifier validation enabled,
        # explicitly enable it here.
        "bsmr": {"skip_cli_modifier_validation_DO_NOT_SET_TO_TRUE_ON_CI": ""}
    },
)


@BSMR_TEST_MARKER
async def test_one_cli_modifier(bsmr: Bsmr) -> None:
    # -m A
    assert CONSTRAINT_A in await get_cfg(bsmr, TARGET, "--modifier", CONSTRAINT_A)


@BSMR_TEST_MARKER
async def test_two_cli_modifier(bsmr: Bsmr) -> None:
    # -m A,B
    result = await get_cfg(
        bsmr, TARGET, "--modifier", CONSTRAINT_A, "--modifier", CONSTRAINT_B
    )
    assert CONSTRAINT_A in result
    assert CONSTRAINT_B in result


@BSMR_TEST_MARKER
async def test_cli_modifiers_bad_input(bsmr: Bsmr) -> None:
    # -m A B (error)
    await expect_failure(
        bsmr.cquery(f"deps({TARGET})", "--modifier", CONSTRAINT_A, CONSTRAINT_B),
        stderr_regex=f"got args `{CONSTRAINT_B}`",
    )


@BSMR_TEST_MARKER
async def test_cli_modifier_alias(bsmr: Bsmr) -> None:
    assert "ovr_config//os/constraints:linux" in await get_cfg(
        bsmr, TARGET, "--modifier", "linux"
    )
