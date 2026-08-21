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


@bsmr_test()
async def test_optin_inside_consumer_can_depend_on_public_target(
    bsmr: Bsmr,
) -> None:
    # PUBLIC clipped to cap; inside consumer matches.
    await bsmr.ctargets("root//intersect/inside_consumer:c")


@bsmr_test()
async def test_optin_clips_public_target_for_outside_consumer(
    bsmr: Bsmr,
) -> None:
    # PUBLIC silently clipped (not rejected); outside consumer fails.
    # Locks the diagnostic: error mentions visibility attr, cap, and the function name.
    result = await expect_failure(
        bsmr.ctargets("root//outside_consumer:c"),
        stderr_regex=r"is not visible to.*visibility = .*Capped to.*enforce_visibility_intersection",
    )
    golden(
        output=sanitize_stderr(result.stderr),
        rel_path="golden/test_optin_clips_public_target_for_outside_consumer.golden.txt",
    )


@bsmr_test()
async def test_optin_cap_blocks_target_visibility_leaking_outside_cap(
    bsmr: Bsmr,
) -> None:
    # Target's own `visibility` lists `leak_destination/...`; cap blocks it.
    # Differs from `enforce_strict_visibility` which would allow this leak.
    result = await expect_failure(
        bsmr.ctargets("root//leak_destination/consumer:c"),
        stderr_regex=r"is not visible to.*Capped to.*enforce_visibility_intersection",
    )
    golden(
        output=sanitize_stderr(result.stderr),
        rel_path="golden/test_optin_cap_blocks_target_visibility_leaking_outside_cap.golden.txt",
    )


@bsmr_test()
async def test_optin_target_own_visibility_match_passes(bsmr: Bsmr) -> None:
    # Consumer matches both visibility attr and cap.
    await bsmr.ctargets("root//intersect/sub_b/consumer:c")


@bsmr_test()
async def test_inherit_true_child_can_still_tighten_cap(bsmr: Bsmr) -> None:
    # Regression: with `inherit=True`, the child contributes its EXPLICIT
    # `visibility=B` to the cap (not `parent.visibility ∪ B`), so a
    # tighter child cap is not silently absorbed into the parent's.
    result = await expect_failure(
        bsmr.ctargets("root//inherit_test/other/consumer:c"),
        stderr_regex=r"is not visible to",
    )
    golden(
        output=sanitize_stderr(result.stderr),
        rel_path="golden/test_inherit_true_child_can_still_tighten_cap.golden.txt",
    )
    await bsmr.ctargets("root//inherit_test/restricted_child/inside/consumer:c")


@bsmr_test()
async def test_package_with_omitted_visibility_does_not_empty_cap(
    bsmr: Bsmr,
) -> None:
    # Regression: `package(inherit=True, within_view=[...])` (no `visibility=`)
    # combined with `enforce_visibility_intersection()` must NOT contribute an
    # empty list to the cap. Before the fix, the omitted `visibility=` defaulted
    # to `[]` and was treated as an explicit empty contribution, intersecting
    # the cap down to the empty set and blocking all consumers.
    await bsmr.ctargets("root//inherit_test/no_vis_child/inside_consumer:c")


@bsmr_test()
async def test_optin_preserves_parent_within_view(bsmr: Bsmr) -> None:
    # Regression: opt-in must not widen the inherited `within_view` to PUBLIC
    # (would happen if implementation routed through `package(...)`).
    await bsmr.ctargets("root//within_view_preserve/child/ok_dep:c")
    result = await expect_failure(
        bsmr.ctargets("root//within_view_preserve/child_bad/bad_dep:c"),
        stderr_regex=r"within_view",
    )
    golden(
        output=sanitize_stderr(result.stderr),
        rel_path="golden/test_optin_preserves_parent_within_view.golden.txt",
    )


@bsmr_test()
async def test_call_from_bzl_is_rejected(bsmr: Bsmr) -> None:
    result = await expect_failure(
        bsmr.ctargets("root//indirect_call/leaf:t"),
        stderr_regex=r"`enforce_visibility_intersection\(\)` can only be called from a `PACKAGE` file",
    )
    golden(
        output=sanitize_stderr(result.stderr),
        rel_path="golden/test_call_from_bzl_is_rejected.golden.txt",
    )
