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

import random
import string

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


@bsmr_test()
async def test_validation_concurrent(bsmr: Bsmr) -> None:
    # There are 2 actions — slow build action and fast validation action.
    # Check that validation doesn't wait for a slow DefaultInfo artifact to be built and fails the build first.
    await expect_failure(
        bsmr.build(
            ":plate",
            "-c",
            f"test.cache_buster={_random_string()}",
        ),
        stderr_regex="Validation for `.+` failed",
    )


def _random_string() -> str:
    return "".join(random.choice(string.ascii_lowercase) for _ in range(256))
