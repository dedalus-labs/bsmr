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
from bsmr.tests.e2e_util.helper.golden import golden


async def _test_analysis_query_invalidation_impl(bsmr: Bsmr, name: str) -> None:
    linux = await bsmr.build_without_report(
        ":root", "-c", "test.configuration=linux", "--out=-"
    )
    macos = await bsmr.build_without_report(
        ":root", "-c", "test.configuration=macos", "--out=-"
    )

    golden(
        output=linux.stdout,
        rel_path=f"{name}/linux.txt.golden",
    )
    golden(
        output=macos.stdout,
        rel_path=f"{name}/macos.txt.golden",
    )

    # Mostly here to really be safe but in practice this fails with an
    # incompatible target earlier if we have a bug.
    assert "linux-select-dep" in linux.stdout
    assert "macos-select-dep" in macos.stdout


@bsmr_test(data_dir="analysis_query_invalidation")
async def test_analysis_query_invalidation_deps(bsmr: Bsmr) -> None:
    """
    This is a regression test for T133069783.
    """
    await _test_analysis_query_invalidation_impl(
        bsmr, name="analysis_query_invalidation"
    )


@bsmr_test(
    data_dir="analysis_query_invalidation_classpath",
)
async def test_analysis_query_invalidation_classpath(bsmr: Bsmr) -> None:
    """
    Equivalent of T133069783 for `classpath()` instead of `deps()` queries.
    """
    await _test_analysis_query_invalidation_impl(
        bsmr, name="analysis_query_invalidation_classpath"
    )


@bsmr_test(data_dir="analysis_query_deps")
async def test_analysis_query_deps(bsmr: Bsmr) -> None:
    deps = await bsmr.build_without_report(":deps", "--out=-")
    golden(
        output=deps.stdout,
        rel_path="analysis_query_deps/deps.txt.golden",
    )
    assert ":foo" in deps.stdout
    assert ":bar" in deps.stdout
    assert ":baz" in deps.stdout
    assert ":qux" in deps.stdout


@bsmr_test(data_dir="analysis_query_deps")
async def test_analysis_query_deps_with_depth(bsmr: Bsmr) -> None:
    deps = await bsmr.build_without_report(":deps1", "--out=-")
    golden(output=deps.stdout, rel_path="analysis_query_deps/deps1.txt.golden")
    assert ":foo" in deps.stdout
    assert ":bar" in deps.stdout
    assert ":baz" in deps.stdout
    assert ":qux" not in deps.stdout


@bsmr_test(setup_eden=True, data_dir="analysis_query_deps")
async def test_analysis_query_target_deps(bsmr: Bsmr) -> None:
    deps = await bsmr.build_without_report(":target_deps", "--out=-")
    golden(
        output=deps.stdout,
        rel_path="analysis_query_deps/target_deps.txt.golden",
    )
    assert ":foo" in deps.stdout
    assert ":bar" in deps.stdout
    assert ":baz" not in deps.stdout
    assert ":qux" not in deps.stdout
