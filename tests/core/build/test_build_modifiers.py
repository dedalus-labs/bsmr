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
import json

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test
from bsmr.tests.e2e_util.helper.golden import golden, GOLDEN_DIRECTORY


@bsmr_test()
async def test_build_with_single_modifier(bsmr: Bsmr) -> None:
    target_with_modifiers = "root//:dummy?root//:macos"

    result = await bsmr.build(target_with_modifiers)

    output = json.loads(result.stdout)

    [configuration] = output["results"][target_with_modifiers]["configured"].keys()

    cfg = await bsmr.audit_configurations(configuration)

    assert "root//:macos" in cfg.stdout


@bsmr_test()
async def test_build_with_multiple_modifiers(bsmr: Bsmr) -> None:
    target_with_modifiers = "root//:dummy?root//:macos+root//:arm"
    result = await bsmr.build(target_with_modifiers)

    output = json.loads(result.stdout)

    [configuration] = output["results"][target_with_modifiers]["configured"].keys()

    cfg = await bsmr.audit_configurations(configuration)

    assert "root//:macos" in cfg.stdout
    assert "root//:arm" in cfg.stdout


@bsmr_test()
async def test_build_order_of_modifiers(bsmr: Bsmr) -> None:
    # if passing in modifiers of the same constraint setting,
    # the last one should be the one that applies
    target_with_modifiers = "root//:dummy?root//:linux+root//:macos"
    result = await bsmr.build(target_with_modifiers)

    output = json.loads(result.stdout)

    [configuration] = output["results"][target_with_modifiers]["configured"].keys()

    cfg = await bsmr.audit_configurations(configuration)

    assert "root//:macos" in cfg.stdout
    assert "root//:linux" not in cfg.stdout


@bsmr_test()
async def test_build_with_different_targets_and_modifiers(bsmr: Bsmr) -> None:
    mac_target = "root//:dummy?root//:macos"
    linux_target = "root//:dummy2?root//:linux"

    result = await bsmr.build(mac_target, linux_target)

    output = json.loads(result.stdout)

    [configuration] = output["results"][mac_target]["configured"].keys()
    cfg = await bsmr.audit_configurations(configuration)
    assert "root//:macos" in cfg.stdout

    [configuration] = output["results"][linux_target]["configured"].keys()
    cfg = await bsmr.audit_configurations(configuration)
    assert "root//:linux" in cfg.stdout


@bsmr_test()
async def test_build_with_same_target_different_modifiers(bsmr: Bsmr) -> None:
    mac_target = "root//:dummy?root//:macos"
    linux_target = "root//:dummy?root//:linux"

    result = await bsmr.build(mac_target, linux_target)

    output = json.loads(result.stdout)

    [configuration] = output["results"][mac_target]["configured"].keys()
    cfg = await bsmr.audit_configurations(configuration)
    assert "root//:macos" in cfg.stdout

    [configuration] = output["results"][linux_target]["configured"].keys()
    cfg = await bsmr.audit_configurations(configuration)
    assert "root//:linux" in cfg.stdout


@bsmr_test()
async def test_build_with_same_target_and_modifiers(bsmr: Bsmr) -> None:
    target_with_modifier = "root//:dummy?root//:macos"
    result = await bsmr.build(target_with_modifier, target_with_modifier)

    output = json.loads(result.stdout)

    [configuration] = output["results"][target_with_modifier]["configured"].keys()

    cfg = await bsmr.audit_configurations(configuration)
    assert "root//:macos" in cfg.stdout


@bsmr_test()
async def test_build_with_target_universe(bsmr: Bsmr) -> None:
    result = await bsmr.build(
        "root//:dummy",
        "--target-universe",
        "root//:universe?root//:linux",
    )

    output = json.loads(result.stdout)

    [configuration] = output["results"]["root//:dummy"]["configured"].keys()

    cfg = await bsmr.audit_configurations(configuration)

    assert "root//:linux" in cfg.stdout


@bsmr_test()
async def test_build_with_target_universe_multiple_modifiers(bsmr: Bsmr) -> None:
    result = await bsmr.build(
        "root//:dummy",
        "--target-universe",
        "root//:universe?root//:linux+root//:arm",
    )

    output = json.loads(result.stdout)

    [configuration] = output["results"]["root//:dummy"]["configured"].keys()

    cfg = await bsmr.audit_configurations(configuration)

    assert "root//:linux" in cfg.stdout
    assert "root//:arm" in cfg.stdout


@bsmr_test()
async def test_build_with_mutliple_target_universes(bsmr: Bsmr) -> None:
    result = await bsmr.build(
        "root//:dummy",
        "--target-universe",
        "root//:universe?root//:linux,root//:dummy?root//:macos+root//:arm",
    )

    output = json.loads(result.stdout)

    configurations = output["results"]["root//:dummy"]["configured"].keys()

    assert len(configurations) == 2

    linux_found = False
    macos_found = False
    for configuration in configurations:
        cfg = await bsmr.audit_configurations(configuration)
        if "root//:linux" in cfg.stdout:
            linux_found = True
        if "root//:macos" in cfg.stdout and "root//:arm" in cfg.stdout:
            macos_found = True

    assert linux_found
    assert macos_found


@bsmr_test()
async def test_build_with_package_pattern(bsmr: Bsmr) -> None:
    result = await bsmr.build("root//:?root//:macos")

    output = json.loads(result.stdout)

    [configuration] = output["results"]["root//:dummy?root//:macos"][
        "configured"
    ].keys()
    cfg = await bsmr.audit_configurations(configuration)
    assert "root//:macos" in cfg.stdout

    [configuration] = output["results"]["root//:dummy2?root//:macos"][
        "configured"
    ].keys()
    cfg = await bsmr.audit_configurations(configuration)
    assert "root//:macos" in cfg.stdout


@bsmr_test()
async def test_build_with_recursive_pattern(bsmr: Bsmr) -> None:
    result = await bsmr.build("root//...?root//:macos")

    output = json.loads(result.stdout)

    [configuration] = output["results"]["root//:dummy?root//:macos"][
        "configured"
    ].keys()
    cfg = await bsmr.audit_configurations(configuration)
    assert "root//:macos" in cfg.stdout

    [configuration] = output["results"]["root//:dummy2?root//:macos"][
        "configured"
    ].keys()
    cfg = await bsmr.audit_configurations(configuration)
    assert "root//:macos" in cfg.stdout

    [configuration] = output["results"][
        "root//recursive_pattern:recursive_target?root//:macos"
    ]["configured"].keys()
    cfg = await bsmr.audit_configurations(configuration)
    assert "root//:macos" in cfg.stdout


@bsmr_test()
async def test_build_fails_with_global_modifiers(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.build("--modifier", "root//:macos", "root//:dummy?root//:linux"),
        stderr_regex=r"Cannot specify modifiers with \?modifier syntax when global CLI modifiers are set with --modifier flag",
    )

    await expect_failure(
        bsmr.build(
            "--modifier",
            "root//:macos",
            "root//:dummy",
            "--target-universe",
            "root//:dummy?root//:linux",
        ),
        stderr_regex=r"Cannot specify modifiers with \?modifier syntax when global CLI modifiers are set with --modifier flag",
    )


@bsmr_test()
async def test_build_fails_with_pattern_modifier_and_target_universe_modifier(
    bsmr: Bsmr,
) -> None:
    await expect_failure(
        bsmr.build(
            "root//:dummy?root//:macos",
            "--target-universe",
            "root//:dummy?root//:linux",
        ),
        stderr_regex=r"Cannot use \?modifier syntax in target pattern expression with --target-universe flag",
    )


async def run_all_output_flags(bsmr: Bsmr, *argv: str) -> str:
    flags = [
        "--show-output",
        "--show-full-output",
        "--show-simple-output",
        "--show-full-simple-output",
        "--show-json-output",
        "--show-full-json-output",
    ]

    results = []
    for flag in flags:
        result = await bsmr.build_without_report(flag, *argv)
        results.append(f"{flag}\n{result.stdout}")

    output = "\n\n".join(results)
    output = output.replace("\\\\", "\\")  # Windows path separators in json
    output = output.replace(str(bsmr.cwd), "/abs/project/root")
    output = output.replace("\\", "/")  # Windows path separators not in json

    return output


@bsmr_test()
async def test_build_modifiers_output_single_modifier(bsmr: Bsmr) -> None:
    result = await run_all_output_flags(
        bsmr,
        "root//:dummy?root//:macos",
    )

    golden(
        output=result,
        rel_path=GOLDEN_DIRECTORY
        + "test_build_modifiers_output_single_modifier.golden.txt",
    )


@bsmr_test()
async def test_build_modifiers_output_multiple_modifiers(bsmr: Bsmr) -> None:
    result = await run_all_output_flags(
        bsmr,
        "root//:dummy?root//:macos+root//:arm",
    )

    golden(
        output=result,
        rel_path=GOLDEN_DIRECTORY
        + "test_build_modifiers_output_multiple_modifiers.golden.txt",
    )


@bsmr_test()
async def test_build_modifiers_output_multiple_patterns(
    bsmr: Bsmr,
) -> None:
    result = await run_all_output_flags(
        bsmr, "root//:dummy?root//:macos", "root//:dummy?root//:linux"
    )

    golden(
        output=result,
        rel_path=GOLDEN_DIRECTORY
        + "test_build_modifiers_output_multiple_patterns.golden.txt",
    )


@bsmr_test()
async def test_build_modifiers_output_multiple_modifiers_multiple_patterns(
    bsmr: Bsmr,
) -> None:
    result = await run_all_output_flags(
        bsmr,
        "root//:dummy?root//:macos+root//:arm",
        "root//:dummy?root//:linux",
    )

    golden(
        output=result,
        rel_path=GOLDEN_DIRECTORY
        + "test_build_modifiers_output_multiple_modifiers_multiple_patterns.golden.txt",
    )


@bsmr_test()
async def test_build_modifiers_output_duplicate_patterns(
    bsmr: Bsmr,
) -> None:
    # Note: switching the order of the modifiers will make it so that both patterns are still in the output
    result = await run_all_output_flags(
        bsmr,
        "root//:dummy?root//:macos+root//:arm",
        "root//:dummy?root//:macos+root//:arm",
    )

    golden(
        output=result,
        rel_path=GOLDEN_DIRECTORY
        + "test_build_modifiers_output_duplicate_patterns.golden.txt",
    )


@bsmr_test()
async def test_build_modifiers_output_with_target_universe(
    bsmr: Bsmr,
) -> None:
    # Modifiers defined in target universe should not be included in the output
    result = await run_all_output_flags(
        bsmr,
        "root//:dummy",
        "--target-universe",
        "root//:dummy?root//:macos+root//:linux",
    )

    golden(
        output=result,
        rel_path=GOLDEN_DIRECTORY
        + "test_build_modifiers_output_with_target_universe.golden.txt",
    )
