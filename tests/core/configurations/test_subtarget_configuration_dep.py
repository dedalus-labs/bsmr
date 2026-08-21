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


@bsmr_test()
async def test_default_target_platform_is_subtarget(bsmr: Bsmr) -> None:
    # FIXME(JakobDegen): Bug. The target specifies a subtarget that does have an appropriate
    # provider.
    await expect_failure(
        bsmr.cquery(":stub"),
        stderr_regex="Expected `root//:alias_platform` to be a `platform\\(\\)` target",
    )


@bsmr_test()
async def test_subtarget_in_select_key(bsmr: Bsmr) -> None:
    res = await bsmr.uquery(
        "root//:with_constraint_key_dep", "-a", "bsmr.configuration_deps"
    )
    res = json.loads(res.stdout)
    # FIXME(JakobDegen): Bug. `bsmr.deps`-like attributes do not include subtargets
    assert list(res.values())[0]["bsmr.configuration_deps"] == ["root//:cat_alias[sub]"]
