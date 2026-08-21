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


import pytest
from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


@bsmr_test()
@pytest.mark.parametrize("section", ["some", "other"])
@pytest.mark.parametrize("root", ["true", "false"])
async def test_deprecated_config(bsmr: Bsmr, section: str, root: str) -> None:
    _ = await expect_failure(
        bsmr.build(
            f":test_target_{section}_config1",
            "-c",
            f"test.section={section}",
            "-c",
            "test.conf=config1",
            "-c",
            f"test.root={root}",
        ),
        stderr_regex=f"{section}.config1 is no longer used. Please use other.config2",
    )


@bsmr_test()
@pytest.mark.parametrize("root", ["true", "false"])
async def test_not_deprecated_config(bsmr: Bsmr, root: str) -> None:
    section = "other"
    _ = await bsmr.build(
        f":test_target_{section}_config2",
        "-c",
        f"test.section={section}",
        "-c",
        "test.conf=config2",
        "-c",
        f"test.root={root}",
    )


@bsmr_test()
async def test_no_deprecated_cell_config(bsmr: Bsmr) -> None:
    section = "other"
    await bsmr.build(
        f"cell//:test_target_{section}_config1",
        "-c",
        f"test.section={section}",
        "-c",
        "test.conf=config1",
    )


@bsmr_test()
async def test_deprecated_cell_config2(bsmr: Bsmr) -> None:
    section = "other"
    _ = await expect_failure(
        bsmr.build(
            f"cell//:test_target_{section}_config2",
            "-c",
            f"test.section={section}",
            "-c",
            "test.conf=config2",
        ),
        stderr_regex=f"{section}.config2 is no longer used. Please use other.config3",
    )
