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


from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


@bsmr_test()
async def test_run_single_modifier(bsmr: Bsmr) -> None:
    result = await bsmr.run("root//:run?root//:macos")

    [os, cpu] = result.stdout.strip().split()

    assert os == "macos"
    assert cpu == "DEFAULT"


@bsmr_test()
async def test_run_multiple_modifiers(bsmr: Bsmr) -> None:
    result = await bsmr.run("root//:run?root//:macos+root//:arm")

    [os, cpu] = result.stdout.strip().split()

    assert os == "macos"
    assert cpu == "arm"


@bsmr_test()
async def test_run_order_of_modifiers(bsmr: Bsmr) -> None:
    result = await bsmr.run("root//:run?root//:macos+root//:linux")

    [os, cpu] = result.stdout.strip().split()

    assert os == "linux"
    assert cpu == "DEFAULT"


@bsmr_test()
async def test_run_target_universe_single_modifier(bsmr: Bsmr) -> None:
    result = await bsmr.run(
        "root//:run",
        "--target-universe",
        "root//:run?root//:macos",
    )

    [os, cpu] = result.stdout.strip().split()

    assert os == "macos"
    assert cpu == "DEFAULT"


@bsmr_test()
async def test_run_target_universe_multiple_modifiers(bsmr: Bsmr) -> None:
    result = await bsmr.run(
        "root//:run",
        "--target-universe",
        "root//:run?root//:macos+root//:arm",
    )

    [os, cpu] = result.stdout.strip().split()

    assert os == "macos"
    assert cpu == "arm"


@bsmr_test()
async def test_run_fails_with_global_modifiers(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.run(
            "--modifier",
            "root//:macos",
            "root//:run?root//:linux",
        ),
        stderr_regex=r"Cannot specify modifiers with \?modifier syntax when global CLI modifiers are set with --modifier flag",
    )

    await expect_failure(
        bsmr.run(
            "--modifier",
            "root//:macos",
            "root//:run",
            "--target-universe",
            "root//:run?root//:linux",
        ),
        stderr_regex=r"Cannot specify modifiers with \?modifier syntax when global CLI modifiers are set with --modifier flag",
    )


@bsmr_test()
async def test_run_fails_with_pattern_modifier_and_target_universe_modifier(
    bsmr: Bsmr,
) -> None:
    await expect_failure(
        bsmr.run(
            "root//:run?root//:macos",
            "--target-universe",
            "root//:run?root//:arm",
        ),
        stderr_regex=r"Cannot use \?modifier syntax in target pattern expression with --target-universe flag",
    )
