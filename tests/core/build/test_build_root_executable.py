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
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test

"""
Test that when we render paths relative to the repo root, we prefix them with a
`./` to ensure the OS executes the cwd-relative path and doesn't do a $PATH
lookup for them.
"""


@bsmr_test()
async def test_build_root_executable_local(bsmr: Bsmr) -> None:
    await bsmr.build(":top", "--local-only")


@bsmr_test()
async def test_build_root_executable_remote(bsmr: Bsmr) -> None:
    await bsmr.build(":top", "--remote-only")
