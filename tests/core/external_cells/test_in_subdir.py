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


@bsmr_test(setup_eden=True)
async def test_in_subdir(bsmr: Bsmr) -> None:
    err = "No such file or directory"
    await expect_failure(
        bsmr.targets("test_bundled_cell//dir:"),
        stderr_regex=err,
    )
    await expect_failure(
        bsmr.cquery("root//:"),
        stderr_regex=err,
    )
    # FIXME(JakobDegen): Decide if this is a bug or not
    (bsmr.cwd / "somedir").mkdir()
    await bsmr.targets("test_bundled_cell//dir:")
    await bsmr.cquery("root//:")
