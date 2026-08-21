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
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test
from bsmr.tests.e2e_util.helper.golden import golden_replace_cfg_hash, sanitize_stderr


def _replace_hash(s: str) -> str:
    return re.sub(r"\b[0-9a-f]{16}\b", "<HASH>", s)


GOLDEN_DIRECTORY = "modifiers/golden/"


@bsmr_test(data_dir="sorted")
async def test_listed_providers_are_sorted(bsmr: Bsmr) -> None:
    result = await bsmr.audit("providers", "//:target", "--list")

    # "  - DefaultInfo" -> "DefaultInfo"
    providers = [
        line.split("-")[1].strip()
        for line in result.stdout.split("\n")
        if line.strip().startswith("-")
    ]
    assert providers == [
        "AlphaInfo",
        "DefaultInfo",
        "ZetaInfo",
    ]


@bsmr_test(data_dir="universe")
async def test_audit_providers_universe(bsmr: Bsmr) -> None:
    result = await bsmr.audit("providers", "//:aaa", "--quiet")
    assert "root//:aaa (root//:p-aaa#<HASH>)" == _replace_hash(result.stdout.strip())

    result = await bsmr.audit(
        "providers", "//:aaa", "--target-universe=//:bbb", "--quiet"
    )
    assert "root//:aaa (root//:p-bbb#<HASH>)" == _replace_hash(result.stdout.strip())


@bsmr_test(data_dir="modifiers")
async def test_audit_providers_with_single_modifier(bsmr: Bsmr) -> None:
    result = await bsmr.audit("providers", "//:dummy?//:macos")

    golden_replace_cfg_hash(
        output=result.stdout,
        rel_path=GOLDEN_DIRECTORY + "audit_providers_with_single_modifier.golden.txt",
    )


@bsmr_test(data_dir="modifiers")
async def test_audit_providers_with_multiple_target_patterns(bsmr: Bsmr) -> None:
    result = await bsmr.audit("providers", "//:dummy?//:macos", "//:dummy?//:arm")

    golden_replace_cfg_hash(
        output=result.stdout,
        rel_path=GOLDEN_DIRECTORY
        + "audit_providers_with_multiple_target_patterns.golden.txt",
    )


@bsmr_test(data_dir="modifiers")
async def test_audit_providers_with_multiple_modifiers(bsmr: Bsmr) -> None:
    result = await bsmr.audit("providers", "//:dummy?//:macos+//:arm")

    golden_replace_cfg_hash(
        output=result.stdout,
        rel_path=GOLDEN_DIRECTORY
        + "audit_providers_with_multiple_modifiers.golden.txt",
    )


@bsmr_test(data_dir="modifiers")
async def test_audit_providers_order_of_modifiers(bsmr: Bsmr) -> None:
    # if passing in modifiers of the same constraint setting,
    # the last one should be the one that applies
    result = await bsmr.audit("providers", "//:dummy?//:macos+//:linux")

    golden_replace_cfg_hash(
        output=result.stdout,
        rel_path=GOLDEN_DIRECTORY + "audit_providers_order_of_modifiers.golden.txt",
    )


@bsmr_test(data_dir="modifiers")
async def test_audit_providers_all_targets_with_modifiers(bsmr: Bsmr) -> None:
    result = await bsmr.audit("providers", "//:?//:macos")

    golden_replace_cfg_hash(
        output=result.stdout,
        rel_path=GOLDEN_DIRECTORY
        + "audit_providers_all_targets_with_modifiers.golden.txt",
    )


@bsmr_test(data_dir="modifiers")
async def test_audit_providers_recursive_with_modifiers(bsmr: Bsmr) -> None:
    result = await bsmr.audit("providers", "//...?//:macos")

    golden_replace_cfg_hash(
        output=result.stdout,
        rel_path=GOLDEN_DIRECTORY
        + "audit_providers_recursive_with_modifiers.golden.txt",
    )


@bsmr_test(data_dir="modifiers")
async def test_audit_providers_modifiers_with_subtarget(bsmr: Bsmr) -> None:
    result = await bsmr.audit("providers", "//:dummy_with_subtarget[sub]?//:macos")

    golden_replace_cfg_hash(
        output=result.stdout,
        rel_path=GOLDEN_DIRECTORY
        + "audit_providers_modifiers_with_subtarget.golden.txt",
    )


@bsmr_test(data_dir="modifiers")
async def test_audit_providers_modifiers_with_target_universe(bsmr: Bsmr) -> None:
    result = await bsmr.audit(
        "providers", "//:dummy", "--target-universe", "//:dummy?//:linux"
    )

    golden_replace_cfg_hash(
        output=result.stdout,
        rel_path=GOLDEN_DIRECTORY
        + "audit_providers_modifiers_with_target_universe.golden.txt",
    )


@bsmr_test(data_dir="modifiers")
async def test_audit_providers_modifiers_with_multiple_target_universe(
    bsmr: Bsmr,
) -> None:
    result = await bsmr.audit(
        "providers",
        "//:dummy",
        "--target-universe",
        "//:dummy?//:linux,//:dummy?//:macos",
    )

    golden_replace_cfg_hash(
        output=result.stdout,
        rel_path=GOLDEN_DIRECTORY
        + "audit_providers_modifiers_with_multiple_target_universe.golden.txt",
    )


@bsmr_test(data_dir="modifiers")
async def test_audit_providers_modifiers_fail_with_global(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.audit("providers", "--modifier", "//:linux", "//:dummy?//:arm"),
        stderr_regex=r"Cannot specify modifiers with \?modifier syntax when global CLI modifiers are set with --modifier flag",
    )

    await expect_failure(
        bsmr.audit(
            "providers",
            "--modifier",
            "//:linux",
            "//:dummy",
            "--target-universe",
            "//:dummy?//:arm",
        ),
        stderr_regex=r"Cannot specify modifiers with \?modifier syntax when global CLI modifiers are set with --modifier flag",
    )


@bsmr_test(data_dir="modifiers")
async def test_audit_providers_modifiers_fail_with_pattern_modifier_and_target_universe_modifier(
    bsmr: Bsmr,
) -> None:
    await expect_failure(
        bsmr.audit(
            "providers", "//:dummy?//:macos", "--target-universe", "//:dummy?//:linux"
        ),
        stderr_regex=r"Cannot use \?modifier syntax in target pattern expression with --target-universe flag",
    )


FILTER_GOLDEN_DIRECTORY = "filter/golden/"


@bsmr_test(data_dir="filter")
async def test_audit_providers_filter_single(bsmr: Bsmr) -> None:
    result = await bsmr.audit("providers", "-p", "FooInfo", "//:has_all")

    golden_replace_cfg_hash(
        output=result.stdout,
        rel_path=FILTER_GOLDEN_DIRECTORY + "audit_providers_filter_single.golden.txt",
    )


@bsmr_test(data_dir="filter")
async def test_audit_providers_filter_multiple(bsmr: Bsmr) -> None:
    result = await bsmr.audit(
        "providers", "-p", "FooInfo", "-p", "BarInfo", "//:has_all"
    )

    golden_replace_cfg_hash(
        output=result.stdout,
        rel_path=FILTER_GOLDEN_DIRECTORY + "audit_providers_filter_multiple.golden.txt",
    )


@bsmr_test(data_dir="filter")
async def test_audit_providers_filter_not_found(bsmr: Bsmr) -> None:
    result = await bsmr.audit(
        "providers", "-p", "Nonexistent1", "-p", "Nonexistent2", "//:has_all"
    )

    golden_replace_cfg_hash(
        output=result.stdout,
        rel_path=FILTER_GOLDEN_DIRECTORY
        + "audit_providers_filter_not_found_stdout.golden.txt",
    )
    golden_replace_cfg_hash(
        output=sanitize_stderr(result.stderr),
        rel_path=FILTER_GOLDEN_DIRECTORY
        + "audit_providers_filter_not_found_stderr.golden.txt",
    )


@bsmr_test(data_dir="filter")
async def test_audit_providers_filter_multi_target(bsmr: Bsmr) -> None:
    result = await bsmr.audit(
        "providers", "-p", "FooInfo", "-p", "BarInfo", "//:has_all", "//:has_foo"
    )

    golden_replace_cfg_hash(
        output=result.stdout,
        rel_path=FILTER_GOLDEN_DIRECTORY
        + "audit_providers_filter_multi_target_stdout.golden.txt",
    )
    golden_replace_cfg_hash(
        output=sanitize_stderr(result.stderr),
        rel_path=FILTER_GOLDEN_DIRECTORY
        + "audit_providers_filter_multi_target_stderr.golden.txt",
    )
