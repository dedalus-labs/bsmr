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


from bsmr.tests.e2e_util.api.buck import Buck
from bsmr.tests.e2e_util.buck_workspace import buck_test
from bsmr.tests.e2e_util.helper.golden import golden


@buck_test()
async def test_help(buck: Buck) -> None:
    result = await buck.help_env()
    golden(
        output=result.stdout,
        rel_path="bsmr-help-env.golden.txt",
    )
    result = await buck.help_env("--self-testing")
    golden(
        output=result.stdout,
        rel_path="bsmr-help-env-testing.golden.txt",
    )
