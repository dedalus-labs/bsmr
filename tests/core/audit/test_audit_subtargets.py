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
from bsmr.tests.e2e_util.helper.golden import golden


@bsmr_test()
async def test_audit_subtargets_basic(bsmr: Bsmr) -> None:
    result = await bsmr.audit("subtargets", "//:no_subtargets")
    assert result.stdout == ""

    result = await bsmr.audit("subtargets", "//:foo")
    assert [
        "root//:foo[bar] (<unspecified>)",
        "root//:foo[baz] (<unspecified>)",
    ] == result.stdout.splitlines()


@bsmr_test()
async def test_audit_subtargets_of_subtarget(bsmr: Bsmr) -> None:
    result = await bsmr.audit("subtargets", "//:nested[sub1]")
    assert [
        "root//:nested[sub1][sub2] (<unspecified>)",
        "root//:nested[sub1][sub3] (<unspecified>)",
    ] == result.stdout.splitlines()

    result = await bsmr.audit("subtargets", "//:nested[sub4]")
    assert result.stdout == ""


@bsmr_test()
async def test_audit_subtargets_shallow(bsmr: Bsmr) -> None:
    result = await bsmr.audit("subtargets", "//:deeply_nested", "--shallow")
    assert [
        "root//:deeply_nested[sub1] (<unspecified>)",
        "root//:deeply_nested[sub2] (<unspecified>)",
    ] == result.stdout.splitlines()

    result = await bsmr.audit("subtargets", "//:deeply_nested")
    assert [
        "root//:deeply_nested[sub1] (<unspecified>)",
        "root//:deeply_nested[sub2] (<unspecified>)",
        "root//:deeply_nested[sub2][sub3] (<unspecified>)",
        "root//:deeply_nested[sub2][sub3][sub4] (<unspecified>)",
        "root//:deeply_nested[sub2][sub5] (<unspecified>)",
    ] == result.stdout.splitlines()

    result = await bsmr.audit("subtargets", "//:deeply_nested[sub2]", "--shallow")
    assert [
        "root//:deeply_nested[sub2][sub3] (<unspecified>)",
        "root//:deeply_nested[sub2][sub5] (<unspecified>)",
    ] == result.stdout.splitlines()

    result = await bsmr.audit("subtargets", "//:deeply_nested[sub2]")
    assert [
        "root//:deeply_nested[sub2][sub3] (<unspecified>)",
        "root//:deeply_nested[sub2][sub3][sub4] (<unspecified>)",
        "root//:deeply_nested[sub2][sub5] (<unspecified>)",
    ] == result.stdout.splitlines()


@bsmr_test()
async def test_audit_subtargets_json(bsmr: Bsmr) -> None:
    result = await bsmr.audit("subtargets", "//:no_subtargets", "--json")
    golden(output=result.stdout, rel_path="json/golden.has_no_subtargets.json")

    result = await bsmr.audit("subtargets", "//:foo", "--json")
    golden(output=result.stdout, rel_path="json/golden.basic.json")

    result = await bsmr.audit("subtargets", "//:nested[sub1]", "--json")
    golden(output=result.stdout, rel_path="json/golden.nested.json")

    result = await bsmr.audit("subtargets", "//:deeply_nested", "--json")
    golden(output=result.stdout, rel_path="json/golden.deeply_nested_basic.json")

    result = await bsmr.audit("subtargets", "//:deeply_nested[sub2]", "--json")
    golden(output=result.stdout, rel_path="json/golden.deeply_nested_subs_of_sub.json")


def _extract_configuration(s: str) -> list[str]:
    return re.findall(r"\((.*?)\)", s)


@bsmr_test()
async def test_audit_subtarget_modifiers(bsmr: Bsmr) -> None:
    result = await bsmr.audit(
        "subtargets",
        "//:deeply_nested?root//:linux",
    )

    cfgs = _extract_configuration(result.stdout)
    for cfg in cfgs:
        cfg = await bsmr.audit_configurations(cfg)
        assert "root//:linux" in cfg.stdout

    result = await bsmr.audit(
        "subtargets",
        "//:nested?root//:macos",
        "//:deeply_nested?root//:linux",
        "--json",
    )

    [linux_cfg, macos_cfg] = _extract_configuration(result.stdout)

    linux_cfg = await bsmr.audit_configurations(linux_cfg)
    assert "root//:linux" in linux_cfg.stdout

    macos_cfg = await bsmr.audit_configurations(macos_cfg)
    assert "root//:macos" in macos_cfg.stdout

    result = await bsmr.audit(
        "subtargets",
        "//:foo?root//:macos+root//:arm",
        "//:foo?root//:arm+root//:macos",
        "--json",
    )

    [cfg] = _extract_configuration(result.stdout)

    cfg = await bsmr.audit_configurations(cfg)
    assert "root//:macos" in cfg.stdout


@bsmr_test()
async def test_audit_subtarget_modifiers_target_universe(bsmr: Bsmr) -> None:
    result = await bsmr.audit(
        "subtargets",
        "//:deeply_nested",
        "--target-universe",
        "//:deeply_nested?root//:linux",
    )

    cfgs = _extract_configuration(result.stdout)
    for cfg in cfgs:
        cfg = await bsmr.audit_configurations(cfg)
        assert "root//:linux" in cfg.stdout

    result = await bsmr.audit(
        "subtargets",
        "//:deeply_nested",
        "--target-universe",
        "//:deeply_nested?root//:linux,//:deeply_nested?root//:macos",
        "--json",
    )

    cfgs = _extract_configuration(result.stdout)
    assert len(cfgs) == 2

    # Audit each configuration to determine which is linux and which is macos
    linux_cfg = None
    macos_cfg = None

    for cfg in cfgs:
        audited_cfg = await bsmr.audit_configurations(cfg)
        if "root//:linux" in audited_cfg.stdout:
            linux_cfg = audited_cfg
        elif "root//:macos" in audited_cfg.stdout:
            macos_cfg = audited_cfg

    assert linux_cfg is not None, "Could not find linux configuration"
    assert macos_cfg is not None, "Could not find macos configuration"

    result = await bsmr.audit(
        "subtargets",
        "//:foo",
        "--target-universe",
        "//:foo?root//:macos+root//:arm,//:foo?root//:arm+root//:macos",
        "--json",
    )

    [cfg] = _extract_configuration(result.stdout)

    cfg = await bsmr.audit_configurations(cfg)
    assert "root//:macos" in cfg.stdout


@bsmr_test()
async def test_audit_subtarget_fails_with_global_modifiers(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.audit(
            "subtargets",
            "--modifier",
            "root//:linux",
            "//:deeply_nested?root//:macos",
        ),
        stderr_regex=r"Cannot specify modifiers with \?modifier syntax when global CLI modifiers are set with --modifier flag",
    )

    await expect_failure(
        bsmr.audit(
            "subtargets",
            "--modifier",
            "root//:linux",
            "//:deeply_nested",
            "--target-universe",
            "//:deeply_nested?root//:macos",
        ),
        stderr_regex=r"Cannot specify modifiers with \?modifier syntax when global CLI modifiers are set with --modifier flag",
    )


@bsmr_test()
async def test_audit_subtarget_fails_with_pattern_modifier_and_target_universe_modifier(
    bsmr: Bsmr,
) -> None:
    await expect_failure(
        bsmr.audit(
            "subtargets",
            "//:deeply_nested?root//:linux",
            "--target-universe",
            "//:deeply_nested?root//:macos",
        ),
        stderr_regex=r"Cannot use \?modifier syntax in target pattern expression with --target-universe flag",
    )
