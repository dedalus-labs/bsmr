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


@buck_test(inplace=True)
async def test_conflicting_fbcode_coverage_flags_fail(buck: Buck) -> None:
    await expect_failure(
        buck.test(
            *[
                "--config",
                "fbcode.coverage=true",
                "--config",
                "fbcode.coverage_selective=true",
                "upstream//testing_frameworks/code_coverage/playground:test",
            ]
        ),
        stderr_regex=r"""fbcode.coverage and fbcode.coverage_selective are both true. Pick one.""",
    )


@buck_test(inplace=True)
async def test_fbcode_coverage_selective_require_filters(buck: Buck) -> None:
    await expect_failure(
        buck.test(
            *[
                "--config",
                "fbcode.coverage_selective=true",
                "upstream//testing_frameworks/code_coverage/playground:test",
            ]
        ),
        stderr_regex=r"""fbcode.coverage_selective=true with no filters""",
    )
