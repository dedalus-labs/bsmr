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


import os
import tempfile

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test
from bsmr.tests.e2e_util.helper.utils import filter_events


async def check_dice_equality(bsmr: Bsmr) -> None:
    dice_equal = await filter_events(
        bsmr,
        "Event",
        "data",
        "Instant",
        "data",
        "DiceEqualityCheck",
        "is_equal",
    )
    assert len(dice_equal) == 1
    assert dice_equal[0] is True


async def check_config_is_the_same(bsmr: Bsmr) -> None:
    # We only fire this event where there are config invalidations.
    has_new_configs = await filter_events(
        bsmr,
        "Event",
        "data",
        "Instant",
        "data",
        "CellHasNewConfigs",
    )
    assert len(has_new_configs) == 0


async def check_config_is_different(bsmr: Bsmr) -> None:
    # We only fire this event where there are config invalidations.
    has_new_configs = await filter_events(
        bsmr,
        "Event",
        "data",
        "Instant",
        "data",
        "CellHasNewConfigs",
    )
    assert len(has_new_configs) == 1

    assert has_new_configs[0]["cell"] == "root"


@bsmr_test()
async def test_ignore_state_invalidation_with_re_override_in_arg(bsmr: Bsmr) -> None:
    # Add arg to switch to bsmr-user
    await bsmr.build(
        "root//:simple",
        "--config",
        "bsmr_re_client.override_use_case=bsmr-user",
    )
    # No arg, default is bsmr-default
    await bsmr.build("root//:simple")
    await check_dice_equality(bsmr)
    await check_config_is_the_same(bsmr)
    # Add arg to switch to bsmr-user again
    await bsmr.build(
        "root//:simple",
        "--config",
        "bsmr_re_client.override_use_case=bsmr-user",
    )
    await check_dice_equality(bsmr)
    await check_config_is_the_same(bsmr)


@bsmr_test()
async def test_ignore_state_invalidation_with_re_override_in_config(bsmr: Bsmr) -> None:
    # Default is bsmr-default
    await bsmr.build("root//:simple")
    # Add config to switch to bsmr-user
    with open(bsmr.cwd / ".bsmr.local", "w") as f:
        f.write("[bsmr_re_client]\n")
        f.write("override_use_case = bsmr-user\n")
    await bsmr.build("root//:simple")
    await check_config_is_different(bsmr)
    # Add config to return to bsmr-default
    with open(bsmr.cwd / ".bsmr.local", "w") as f:
        f.write("[bsmr_re_client]\n")
        f.write("override_use_case = bsmr-default\n")
    await bsmr.build("root//:simple")
    await check_config_is_different(bsmr)


@bsmr_test()
async def test_ignore_state_invalidation_with_re_override_in_external_config(
    bsmr: Bsmr,
) -> None:
    # Default is bsmr-default
    await bsmr.build("root//:simple")
    # Add config to switch to bsmr-user
    with tempfile.NamedTemporaryFile("w", delete=False) as f:
        f.write("[bsmr_re_client]\n")
        f.write("override_use_case = bsmr-user\n")
        f.close()
        await bsmr.build("root//:simple", "--config-file", f.name)
    await check_config_is_different(bsmr)
    # Add config to return to bsmr-default
    with tempfile.NamedTemporaryFile("w", delete=False) as f:
        f.write("[bsmr_re_client]\n")
        f.write("override_use_case = bsmr-default\n")
        f.close()
        await bsmr.build("root//:simple", "--config-file", f.name)
    await check_config_is_different(bsmr)


@bsmr_test()
async def test_ignore_state_invalidation_with_re_override_in_external_config_source(
    bsmr: Bsmr,
) -> None:
    with tempfile.NamedTemporaryFile("w", delete=False) as temp:
        env = os.environ.copy()
        env["BSMR_TEST_EXTRA_EXTERNAL_CONFIG"] = temp.name

        # Default is bsmr-default
        await bsmr.build("root//:simple", env=env)

        # Add config to switch to bsmr-user
        temp.write("[bsmr_re_client]\n")
        temp.write("override_use_case = bsmr-user\n")
        temp.flush()
        await bsmr.build("root//:simple", env=env)
        await check_config_is_different(bsmr)

        # Add config to return to bsmr-default
        temp.seek(0)
        temp.truncate()
        temp.write("[bsmr_re_client]\n")
        temp.write("override_use_case = bsmr-default\n")
        temp.flush()
        await bsmr.build("root//:simple", env=env)
        await check_dice_equality(bsmr)
