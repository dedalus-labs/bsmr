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
import re

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


def _extract_configuration(s: str) -> list[str]:
    return re.findall(r"\((.*?)\)", s)


@bsmr_test()
async def test_ctargets_modifier_single_pattern(bsmr: Bsmr) -> None:
    result = await bsmr.ctargets("root//:target?root//:macos")

    [configuration] = _extract_configuration(result.stdout)

    macos_cfg = await bsmr.audit_configurations(configuration)

    assert "root//:macos" in macos_cfg.stdout


@bsmr_test()
async def test_ctargets_modifier_multiple_patterns(bsmr: Bsmr) -> None:
    result = await bsmr.ctargets(
        "root//:target?root//:macos", "root//:other_target?root//:macos"
    )

    [target_configuration, other_configuration] = _extract_configuration(result.stdout)

    target_cfg = await bsmr.audit_configurations(target_configuration)
    assert "root//:macos" in target_cfg.stdout

    other_cfg = await bsmr.audit_configurations(other_configuration)
    assert "root//:macos" in other_cfg.stdout


@bsmr_test()
async def test_ctargets_modifier_multiple_modifiers(bsmr: Bsmr) -> None:
    result = await bsmr.ctargets("root//:target?root//:macos+root//:arm")

    [configuration] = _extract_configuration(result.stdout)

    multi_cfg = await bsmr.audit_configurations(configuration)

    assert "root//:macos" in multi_cfg.stdout
    assert "root//:arm" in multi_cfg.stdout


@bsmr_test()
async def test_ctargets_modifier_order_of_modifiers(bsmr: Bsmr) -> None:
    # if passing in modifiers of the same constraint setting,
    # the last one should be the one that applies
    result = await bsmr.ctargets("root//:target?root//:macos+root//:linux")

    [configuration] = _extract_configuration(result.stdout)

    cfg = await bsmr.audit_configurations(configuration)

    assert "root//:linux" in cfg.stdout
    assert "root//:macos" not in cfg.stdout


@bsmr_test()
async def test_ctargets_modifier_multi_target_pattern(bsmr: Bsmr) -> None:
    result = await bsmr.ctargets("root//:?root//:macos")

    [other_configuration, _, configuration] = _extract_configuration(result.stdout)[5:]

    other_cfg = await bsmr.audit_configurations(other_configuration)
    assert "root//:macos" in other_cfg.stdout

    cfg = await bsmr.audit_configurations(configuration)
    assert "root//:macos" in cfg.stdout


@bsmr_test()
async def test_ctargets_modifier_same_target(bsmr: Bsmr) -> None:
    # if the same target has the same modifiers there should only be one instance of it
    result = await bsmr.ctargets(
        "root//:target?root//:macos", "root//:target?root//:macos"
    )

    [configuration] = _extract_configuration(result.stdout)

    cfg = await bsmr.audit_configurations(configuration)

    assert "root//:macos" in cfg.stdout


@bsmr_test()
async def test_ctargets_fails_with_global_modifier(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.ctargets("--modifier", "root//:linux", "root//:target?root//:macos"),
        stderr_regex=r"Cannot specify modifiers with \?modifier syntax when global CLI modifiers are set with --modifier flag",
    )
