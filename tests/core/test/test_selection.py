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
async def test_ok(bsmr: Bsmr) -> None:
    await bsmr.test("//:ok")


@bsmr_test()
async def test_fail(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.test("//:fail"), stderr_regex="Fail: root//:fail - unmanaged"
    )


@bsmr_test()
async def test_tests_attribute(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.test("//:noop_references_fail"),
        stderr_regex="Fail: root//:fail - unmanaged",
    )


@bsmr_test()
async def test_tests_attribute_transitive(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.test(
            "//:noop_transitively_references_fail",
        ),
        stderr_regex="Fail: root//:fail - unmanaged",
    )


@bsmr_test()
async def test_tests_attribute_cycle(bsmr: Bsmr) -> None:
    bsmr.test(
        "//:noop_cycle1",
    )


@bsmr_test()
async def test_tests_attribute_self_transition(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.test("//:noop_self_transition_references_fail"),
        stderr_regex="Fail: root//:fail - unmanaged",
    )
