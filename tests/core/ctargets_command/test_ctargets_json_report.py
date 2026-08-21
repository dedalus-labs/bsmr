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
from bsmr.tests.e2e_util.helper.golden import (
    golden,
    golden_replace_cfg_hash,
    sanitize_stderr,
)


@bsmr_test()
async def test_ctargets_json_report_basic(bsmr: Bsmr) -> None:
    """Test basic --json-report with only compatible targets"""
    result = await bsmr.ctargets(
        "//a:target1",
        "//a:target2",
        "--target-platforms=root//:p",
        "--json-report",
    )

    golden_replace_cfg_hash(
        output=result.stdout,
        rel_path="golden/basic.stdout.golden",
    )
    golden(
        output=sanitize_stderr(result.stderr),
        rel_path="golden/basic.stderr.golden",
    )


@bsmr_test()
async def test_ctargets_json_report_with_incompatible(bsmr: Bsmr) -> None:
    """Test --json-report with incompatible targets"""
    result = await bsmr.ctargets(
        "//a:target1",
        "//a:macos_only",
        "//a:target2",
        "--target-platforms=root//:linux_platform",
        "--json-report",
    )

    golden_replace_cfg_hash(
        output=result.stdout,
        rel_path="golden/with_incompatible.stdout.golden",
    )
    golden(
        output=sanitize_stderr(result.stderr),
        rel_path="golden/with_incompatible.stderr.golden",
    )


@bsmr_test()
async def test_ctargets_json_report_with_transitive_incompatible(bsmr: Bsmr) -> None:
    """Test --json-report with transitively incompatible targets"""
    result = await bsmr.ctargets(
        "//a:target1",
        "//c:depends_on_incompatible",
        "//a:target2",
        "--target-platforms=root//:linux_platform",
        "--keep-going",
        "--json-report",
    )

    golden_replace_cfg_hash(
        output=result.stdout,
        rel_path="golden/transitive_incompatible.stdout.golden",
    )
    golden(
        output=sanitize_stderr(result.stderr),
        rel_path="golden/transitive_incompatible.stderr.golden",
    )


@bsmr_test()
async def test_ctargets_json_report_with_errors_and_keep_going(bsmr: Bsmr) -> None:
    """Test --json-report with errors (should only appear in stderr, not JSON)"""
    result = await bsmr.ctargets(
        "//a:target1",
        "//b:any",
        "//a:target2",
        "--target-platforms=root//:p",
        "--keep-going",
        "--json-report",
    )

    golden_replace_cfg_hash(
        output=result.stdout,
        rel_path="golden/with_errors.stdout.golden",
    )
    golden(
        output=sanitize_stderr(result.stderr),
        rel_path="golden/with_errors.stderr.golden",
    )


@bsmr_test()
async def test_ctargets_json_report_mixed(bsmr: Bsmr) -> None:
    """Test --json-report with mix of compatible, incompatible, and errors"""
    result = await bsmr.ctargets(
        "//a:target1",
        "//a:macos_only",
        "//b:any",
        "//a:target2",
        "//c:depends_on_incompatible",
        "--target-platforms=root//:linux_platform",
        "--keep-going",
        "--json-report",
    )

    golden_replace_cfg_hash(
        output=result.stdout,
        rel_path="golden/mixed.stdout.golden",
    )
    golden(
        output=sanitize_stderr(result.stderr),
        rel_path="golden/mixed.stderr.golden",
    )


@bsmr_test()
async def test_ctargets_json_report_only_incompatible(bsmr: Bsmr) -> None:
    """Test --json-report when all targets are incompatible"""
    result = await bsmr.ctargets(
        "//a:macos_only",
        "--target-platforms=root//:linux_platform",
        "--json-report",
    )

    golden_replace_cfg_hash(
        output=result.stdout,
        rel_path="golden/only_incompatible.stdout.golden",
    )
    golden(
        output=sanitize_stderr(result.stderr),
        rel_path="golden/only_incompatible.stderr.golden",
    )


@bsmr_test()
async def test_ctargets_json_report_with_attributes(bsmr: Bsmr) -> None:
    """Test --json-report with attribute filtering"""
    result = await bsmr.ctargets(
        "//a:target1",
        "//a:target2",
        "--target-platforms=root//:p",
        "--json-report",
        "--output-attribute",
        "^name$",
    )

    golden_replace_cfg_hash(
        output=result.stdout,
        rel_path="golden/with_attributes.stdout.golden",
    )
    golden(
        output=sanitize_stderr(result.stderr),
        rel_path="golden/with_attributes.stderr.golden",
    )
