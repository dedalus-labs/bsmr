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


@bsmr_test()
async def test_run_executable(bsmr: Bsmr) -> None:
    result = await bsmr.run("root//:print_animal_hello")
    assert result.stdout.strip() == "hello dog"

    result = await bsmr.run(
        "root//:print_animal_hello", "--target-universe", "root//:cat_universe"
    )
    assert result.stdout.strip() == "hello cat"


@bsmr_test()
async def test_run_with_transition_without_target_universe(bsmr: Bsmr) -> None:
    result = await bsmr.run(
        "root//:bsmr",
        "--target-platforms=root//:p_cat",
    )

    # The transition (deliberately) loses the configuration so that we get the
    # DEFAULT 'hello bsmr' from the select in the target definition.
    assert result.stdout.strip() == "hello bsmr"


@bsmr_test()
async def test_run_with_transition_with_target_universe(bsmr: Bsmr) -> None:
    result = await bsmr.run(
        "root//:bsmr",
        "--target-platforms=root//:p_cat",
        "--target-universe",
        "root//:bsmr",
    )

    # The transition (deliberately) loses the configuration so that we get the
    # DEFAULT 'hello bsmr' from the select in the target definition.
    assert result.stdout.strip() == "hello bsmr"


@bsmr_test()
async def test_run_target_not_in_universe(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.run(
            "root//:print_animal_hello",
            "--target-universe",
            "root//:print_animal_goodbye",
        ),
        stderr_regex="Target `root//:print_animal_hello` is not found in the specified target universe",
    )
