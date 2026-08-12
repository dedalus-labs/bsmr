# ===----------------------------------------------------------------------===
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


from bsmr.tests.e2e_util.api.buck import Buck
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.buck_workspace import buck_test


@buck_test()
async def test_platform_wrong_label(buck: Buck) -> None:
    await expect_failure(
        buck.build(
            "//...",
        ),
        stderr_regex=r"Platform target `.*` evaluation returned `ProviderInfo` label `.*` which resolved to an unequal configuration",
    )
