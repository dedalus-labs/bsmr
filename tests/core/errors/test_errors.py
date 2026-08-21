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
import re

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test, env
from bsmr.tests.e2e_util.helper.golden import golden, strip_glog_lines


@bsmr_test()
async def test_soft_error(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.targets(":"), stderr_regex="starlark_raised_soft_error.*Will be reported"
    )


@bsmr_test()
@env("BSMR_HARD_ERROR", "false")
async def test_soft_error_quiet(bsmr: Bsmr) -> None:
    res = await bsmr.targets("quiet:", ":")
    assert "starlark_raised_soft_error" in res.stderr
    assert "starlark_quiet_soft_error" not in res.stderr


@bsmr_test()
@env("BSMR_HARD_ERROR", "false")
async def test_soft_error_no_stack(bsmr: Bsmr) -> None:
    res = await bsmr.targets(":")
    assert "Traceback" in res.stderr

    res = await bsmr.targets("no_stack:")
    assert "Traceback" not in res.stderr


@bsmr_test(
    # windows errors are slightly different, just skip for now
    skip_for_os=["windows"],
)
@env("BSMR_HARD_ERROR", "false")
async def test_package_listing_errors(bsmr: Bsmr) -> None:
    outs = []
    for target in [
        # //package_listing/missing does not exist
        "//package_listing/missing/foo/x/y/lmnop:target",
        # //package_listing/ignored is ignored
        "//package_listing/ignored/foo/x/y/lmnop:target",
        # //package_listing/cell is a cell
        "//package_listing/cell/foo/x/y/lmnop:target",
        # //package_listing/missing_targets_file has no TARGETS file
        "//package_listing/missing_targets_file:target",
        # //package_listing/data.file is a file
        "//package_listing/data.file:target",
        "//package_listing/data.file/subdir:target",
        # Missing directory due to typo
        "//package_listings:",
        # Missing directory due to typo in subdirectory
        "//package_listing/typo_test/subdirr:",
        # Missing directory due to being in the wrong cell
        "//something:",
    ]:
        out = await expect_failure(bsmr.uquery(target, "-v=0", "--console=none"))
        stripped_stderr = re.sub(
            "read_dir(.*)", "read_dir(<stripped absolute path>)", out.stderr
        )
        outs.append(stripped_stderr)

    golden(
        output=strip_glog_lines("\n\n\n".join(outs)),
        rel_path="package_listing/expected.golden.out",
    )


@bsmr_test(
    # windows errors are slightly different, just skip for now
    skip_for_os=["windows"],
)
async def test_configured_graph_deps_collapsed_in_errors(bsmr: Bsmr) -> None:
    out = await expect_failure(
        bsmr.cquery(
            "//deps_collapsed:top",
            "-v=0",
            "--console=none",
            "-c",
            "build.execution_platforms=root//deps_collapsed:exec_platforms",
        )
    )
    stderr = re.sub("#[a-f0-9]*\\)", "#00000000)", out.stderr)
    golden(output=stderr, rel_path="deps_collapsed/expected.golden.out")


@bsmr_test(
    # windows errors are slightly different, just skip for now
    skip_for_os=["windows"],
)
async def test_configured_graph_deps_collapsed_in_errors_2(bsmr: Bsmr) -> None:
    out = await expect_failure(
        bsmr.cquery(
            "//deps_collapsed:top",
            "-v=0",
            "--console=none",
            "-c",
            "build.execution_platforms=root//deps_collapsed:exec_platforms",
            "-c",
            "core_test_errors.broken_select_in_toolchain=1",
        )
    )
    stderr = re.sub("#[a-f0-9]*\\)", "#00000000)", out.stderr)
    golden(output=stderr, rel_path="deps_collapsed/expected_2.golden.out")
