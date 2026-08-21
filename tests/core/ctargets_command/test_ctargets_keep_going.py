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
async def test_ctargets_keep_going_parse_error_json(bsmr: Bsmr) -> None:
    """Test that package parse errors appear in JSON output with --keep-going"""
    result = await bsmr.ctargets(
        "//a:target1",
        "//b:any",
        "//a:target2",
        "--target-platforms=root//:p",
        "--keep-going",
        "--json",
    )

    golden_replace_cfg_hash(
        output=result.stdout,
        rel_path="golden/parse_error_json.stdout.golden",
    )
    golden(
        output=sanitize_stderr(result.stderr),
        rel_path="golden/parse_error_json.stderr.golden",
    )


@bsmr_test()
async def test_ctargets_keep_going_parse_error_plain_text(bsmr: Bsmr) -> None:
    """Test that parse errors go to stderr in plain text mode"""
    result = await bsmr.ctargets(
        "//a:target1",
        "//b:any",
        "//a:target2",
        "--target-platforms=root//:p",
        "--keep-going",
    )

    golden_replace_cfg_hash(
        output=result.stdout,
        rel_path="golden/parse_error_plain_text.stdout.golden",
    )
    golden(
        output=sanitize_stderr(result.stderr),
        rel_path="golden/parse_error_plain_text.stderr.golden",
    )


@bsmr_test()
async def test_ctargets_keep_going_missing_package(bsmr: Bsmr) -> None:
    """Test that missing packages are handled with --keep-going"""
    result = await bsmr.ctargets(
        "//a:target1",
        "//nonexistent_package:target",
        "//a:target2",
        "--target-platforms=root//:p",
        "--keep-going",
        "--json",
    )

    golden_replace_cfg_hash(
        output=result.stdout,
        rel_path="golden/missing_package.stdout.golden",
    )
    golden(
        output=sanitize_stderr(result.stderr),
        rel_path="golden/missing_package.stderr.golden",
    )


@bsmr_test()
async def test_ctargets_keep_going_with_incompatible(bsmr: Bsmr) -> None:
    """Test that incompatible targets and errors work together correctly"""
    result = await bsmr.ctargets(
        "//a:target1",
        "//a:macos_only",
        "//b:any",
        "--target-platforms=root//:linux_platform",
        "--keep-going",
        "--json",
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
async def test_ctargets_keep_going_single_error(bsmr: Bsmr) -> None:
    """Test edge case with single failing target"""
    result = await bsmr.ctargets(
        "//b:does_not_matter",
        "--target-platforms=root//:p",
        "--keep-going",
        "--json",
    )

    golden_replace_cfg_hash(
        output=result.stdout,
        rel_path="golden/single_error.stdout.golden",
    )
    golden(
        output=sanitize_stderr(result.stderr),
        rel_path="golden/single_error.stderr.golden",
    )


@bsmr_test()
async def test_ctargets_keep_going_multiple_packages_with_errors(bsmr: Bsmr) -> None:
    """Test errors from multiple different packages"""
    result = await bsmr.ctargets(
        "//a:target1",
        "//b:any",
        "//d:exists",
        "//nonexistent_package:any",
        "--target-platforms=root//:p",
        "--keep-going",
        "--json",
    )

    golden_replace_cfg_hash(
        output=result.stdout,
        rel_path="golden/multiple_packages_with_errors.stdout.golden",
    )
    golden(
        output=sanitize_stderr(result.stderr),
        rel_path="golden/multiple_packages_with_errors.stderr.golden",
    )


@bsmr_test()
async def test_ctargets_keep_going_transitive_incompatible(bsmr: Bsmr) -> None:
    """Test transitive incompatibility"""
    result = await bsmr.ctargets(
        "//a:target1",
        "//c:depends_on_incompatible",
        "//a:target2",
        "--target-platforms=root//:linux_platform",
        "--keep-going",
        "--json",
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
async def test_ctargets_keep_going_modifier_conflict(bsmr: Bsmr) -> None:
    """Test modifier conflict: pattern modifiers + global modifiers"""
    result = await bsmr.ctargets(
        "//a:target1",
        "//a:target2?root//:linux",
        "--modifier",
        "root//:macos",
        "--keep-going",
        "--json",
    )

    golden_replace_cfg_hash(
        output=result.stdout,
        rel_path="golden/modifier_conflict.stdout.golden",
    )
    golden(
        output=sanitize_stderr(result.stderr),
        rel_path="golden/modifier_conflict.stderr.golden",
    )
