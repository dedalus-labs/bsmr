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

from bsmr.tests.e2e.configurations.cfg_constructor.modifiers_util import get_cfg
from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


@bsmr_test(inplace=False)
async def test_cfg_modifiers_attr(bsmr: Bsmr) -> None:
    result = await bsmr.targets(
        "root//:test",
        "--output-attribute=modifiers",
    )

    targets = json.loads(result.stdout)
    assert len(targets) == 1
    target = targets[0]
    target_modifiers = target["modifiers"]
    assert target_modifiers == ["root//:A_1"]


@bsmr_test(inplace=False)
async def test_cfg_modifiers_attr_ctargets(bsmr: Bsmr) -> None:
    result = await get_cfg(
        bsmr,
        "root//:test2",
    )
    assert ":A_1" in result


@bsmr_test(inplace=False)
async def test_metadata_modifiers_is_hard_error(bsmr: Bsmr) -> None:
    result = await expect_failure(bsmr.ctargets("root//:test_metadata_modifiers"))
    assert (
        'sets `metadata["bsmr.cfg_modifiers"]` which is no longer supported'
        in result.stderr
    )
