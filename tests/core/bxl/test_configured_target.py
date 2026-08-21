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


@bsmr_test(data_dir="")
async def test_unwrap_forward(bsmr: Bsmr) -> None:
    await bsmr.bxl("//bxl/configured_target.bxl:unwrap_forward")


@bsmr_test(data_dir="")
async def test_configured_targets_with_modifiers(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//bxl/configured_target.bxl:configured_targets_with_modifiers"
    )
    configurations = [line.strip() for line in result.stdout.splitlines()]
    linux_cfg = await bsmr.audit_configurations(configurations[0])
    assert "root//:linux" in linux_cfg.stdout
    macos_cfg = await bsmr.audit_configurations(configurations[1])
    assert "root//:macos" in macos_cfg.stdout


@bsmr_test(data_dir="")
async def test_strip_cfg(bsmr: Bsmr) -> None:
    await bsmr.bxl("//bxl/configured_target.bxl:strip_cfg")
