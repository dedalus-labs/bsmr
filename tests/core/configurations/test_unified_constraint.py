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
from bsmr.tests.e2e_util.helper.golden import golden, sanitize_stderr


@bsmr_test(allow_soft_errors=True)
async def test_unified_constraint_defination(bsmr: Bsmr) -> None:
    await bsmr.bxl("//test_unified_constraint.bxl:main")


@bsmr_test()
async def test_unified_constraint_miss_default_fail(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.targets("//miss_default:"),
        stderr_regex=".*Missing named-only parameter `default` for call to `constraint`.*",
    )


@bsmr_test()
async def test_unified_constraint_default_not_appear_in_value_fail(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.audit("subtargets", "//default_value_not_appear:"),
        stderr_regex=r""".*default value 'linux' must be one of the declared values: \["macos", "windows"\].*""",
    )


# 'default' as a value is allowed only when it is also the default value (see the `compiler`
# constraint in the root fixture). Here it is a value but NOT the default, so it must fail as ambiguous.
# Distinct test names are used because test discovery treats 'default' and 'DEFAULT' as the same name
# (case-insensitive).
@bsmr_test()
async def test_unified_constraint_reserved_keyword_default_lowercase_fail(
    bsmr: Bsmr,
) -> None:
    await expect_failure(
        bsmr.audit("subtargets", "//reserved_keyword_default_lowercase:"),
        stderr_regex=".*'default' can be used as a constraint value only when it is also the default value.*",
    )


# Test reserved keyword 'DEFAULT' (uppercase)
@bsmr_test()
async def test_unified_constraint_reserved_keyword_default_uppercase_fail(
    bsmr: Bsmr,
) -> None:
    await expect_failure(
        bsmr.audit("subtargets", "//reserved_keyword_default_uppercase:"),
        stderr_regex=".*'DEFAULT' is a reserved keyword and cannot be used as a constraint value.*",
    )


@bsmr_test(allow_soft_errors=True)
async def test_unified_constraint_cfg_transition(bsmr: Bsmr) -> None:
    await bsmr.bxl("//test_unified_constraint.bxl:test_cfg_transition")


@bsmr_test(allow_soft_errors=True)
async def test_unified_constraint_cfg_transition_v2(bsmr: Bsmr) -> None:
    await bsmr.bxl("//test_unified_constraint.bxl:test_cfg_transition_v2")


@bsmr_test(allow_soft_errors=True)
async def test_unified_constraint_for_constraint_v2(bsmr: Bsmr) -> None:
    await bsmr.bxl("//test_unified_constraint.bxl:constraint_v2")


@bsmr_test()
async def test_unified_constraint_single_value_without_flag_fail(
    bsmr: Bsmr,
) -> None:
    res = await expect_failure(
        bsmr.audit("subtargets", "//single_value_no_flag:", "-v0"),
    )
    golden(
        output=sanitize_stderr(res.stderr),
        rel_path="golden/single_value_no_flag.golden.stderr",
    )


@bsmr_test()
async def test_unified_constraint_single_value_with_flag(bsmr: Bsmr) -> None:
    await bsmr.audit("subtargets", "//single_value_with_flag:")


@bsmr_test()
async def test_unified_constraint_zero_values_with_flag_fail(bsmr: Bsmr) -> None:
    res = await expect_failure(
        bsmr.audit("subtargets", "//zero_values_with_flag:", "-v0"),
    )
    golden(
        output=sanitize_stderr(res.stderr),
        rel_path="golden/zero_values_with_flag.golden.stderr",
    )


@bsmr_test()
async def test_unified_constraint_alias_conflict_with_value_fail(
    bsmr: Bsmr,
) -> None:
    res = await expect_failure(
        bsmr.audit("subtargets", "//alias_conflict_with_value:", "-v0"),
    )
    golden(
        output=sanitize_stderr(res.stderr),
        rel_path="golden/alias_conflict_with_value.golden.stderr",
    )


@bsmr_test()
async def test_unified_constraint_alias_value_not_declared_fail(
    bsmr: Bsmr,
) -> None:
    res = await expect_failure(
        bsmr.audit("subtargets", "//alias_value_not_declared:", "-v0"),
    )
    golden(
        output=sanitize_stderr(res.stderr),
        rel_path="golden/alias_value_not_declared.golden.stderr",
    )


@bsmr_test()
async def test_unified_constraint_alias_reserved_keyword_fail(
    bsmr: Bsmr,
) -> None:
    res = await expect_failure(
        bsmr.audit("subtargets", "//alias_reserved_keyword:", "-v0"),
    )
    golden(
        output=sanitize_stderr(res.stderr),
        rel_path="golden/alias_reserved_keyword.golden.stderr",
    )
