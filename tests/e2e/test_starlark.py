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


@buck_test(inplace=True)
async def test_lint_bsmr(buck: Buck) -> None:
    # FIXME(JakobDegen): Reusing `project.ignore` for this is bad, `starlark
    # lint` should have `-I` and `-X` flags like sapling
    await buck.starlark(
        "lint",
        "bsmr",
        "-c",
        "project.ignore=bsmr/tests/e2e,bsmr/tests/core",
    )


@buck_test(inplace=True)
async def test_typecheck_prelude_lightweight(buck: Buck) -> None:
    await buck.starlark("typecheck", "bsmr/prelude/prelude.bzl")


@buck_test(inplace=True)
async def test_typecheck_prelude_compiler(buck: Buck) -> None:
    await buck.uquery("root//:bsmr", "--unstable-typecheck")
