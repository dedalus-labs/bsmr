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
async def test_construction_validation_good(bsmr: Bsmr) -> None:
    await bsmr.targets("//good:")


@bsmr_test()
async def test_construction_validation_bad(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.targets("//bad:"),
        stderr_regex=r"`impl` function signature is incorrect",
    )


@bsmr_test()
async def test_construction_validation_bad_param_types(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.targets("//bad_param_types:"),
        stderr_regex=r"`impl` function signature is incorrect",
    )


@bsmr_test()
async def test_construction_validation_bad_param_types_vnew(bsmr: Bsmr) -> None:
    # FIXME(JakobDegen): Evaluate whether we can implement this. The performance
    # concerns are a bit higher here because the code is hotter.
    await bsmr.build("//bad_param_types_vnew:")


@bsmr_test()
async def test_construction_validation_bad_return_type(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.targets("//bad_return_type:"),
        stderr_regex=r"`impl` function signature is incorrect",
    )
