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

import os

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.api.bsmr_result import BsmrException
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


# This is just a template test case for `bxl_test` to use bsmr's e2e test framework.
# It does not need to be edited for new `bxl_test`.


@bsmr_test(inplace=True)
async def test_bxl(bsmr: Bsmr) -> None:
    args = []

    bsmr_args = os.environ.get("BSMR_ARGS")
    if bsmr_args:
        args += bsmr_args.split(" ")

    bxl_args = os.environ.get("BXL_ARGS")
    if bxl_args:
        args += ["--"] + bxl_args.split(" ")

    try:
        await bsmr.bxl(os.environ["BXL_MAIN"], *args)
    except BsmrException as e:
        # Re-raise with stderr included in the message for better test output
        raise AssertionError(f"BXL failed:\n{e.stderr}") from e
