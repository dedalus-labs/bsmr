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


@bsmr_test(inplace=True)
async def test_lint_bsmr(bsmr: Bsmr) -> None:
    # FIXME(JakobDegen): Reusing `project.ignore` for this is bad, `starlark
    # lint` should have `-I` and `-X` flags like sapling
    await bsmr.starlark(
        "lint",
        "bsmr",
        "-c",
        "project.ignore=bsmr/tests/e2e,bsmr/tests/core",
    )


@bsmr_test(inplace=True)
async def test_typecheck_prelude_lightweight(bsmr: Bsmr) -> None:
    await bsmr.starlark("typecheck", "bsmr/prelude/prelude.bzl")


@bsmr_test(inplace=True)
async def test_typecheck_prelude_compiler(bsmr: Bsmr) -> None:
    await bsmr.uquery("root//:bsmr", "--unstable-typecheck")
