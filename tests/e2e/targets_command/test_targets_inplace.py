# Copyright (c) Meta Platforms, Inc. and affiliates.
#
# This source code is dual-licensed under either the MIT license found in the
# LICENSE-MIT file in the root directory of this source tree or the Apache
# License, Version 2.0 found in the LICENSE-APACHE file in the root directory
# of this source tree. You may select, at your option, one of the
# above-listed licenses.

# pyre-strict


import os

from bsmr.tests.e2e_util.api.buck import Buck
from bsmr.tests.e2e_util.api.buck_result import ExitCodeV2
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.buck_workspace import buck_test, get_mode_from_platform


@buck_test(inplace=True)
async def test_targets(buck: Buck) -> None:
    result = await buck.targets("root//tests/targets/commands:")

    targets = [
        "root//tests/targets/commands:dynamic",
        "root//tests/targets/commands:exported",
        "root//tests/targets/commands:lib",
    ]

    for target in targets:
        assert target in result.stdout


@buck_test(inplace=True)
async def test_targets_errors(buck: Buck) -> None:
    await expect_failure(
        buck.targets(
            "root//tests/targets/commands:",
            "root//tests/targets/non_existent_path:",
        ),
        exit_code=ExitCodeV2.USER_ERROR,
    )


@buck_test(inplace=True)
async def test_explicit_targets_errors(buck: Buck) -> None:
    await expect_failure(
        buck.targets(
            "root//tests/targets/commands:notarealtarget",
        ),
        exit_code=ExitCodeV2.USER_ERROR,
        stderr_regex="Unknown target `notarealtarget` from package `root//tests/targets/commands`",
    )


@buck_test(inplace=True)
async def test_targets_with_config_value(buck: Buck) -> None:
    targets_enabled_result = await buck.targets(
        "--config",
        "user.targets_enabled=true",
        "root//tests/targets/commands:",
    )
    assert (
        "root//tests/targets/commands:config_defined_target"
        in targets_enabled_result.stdout
    )

    targets_disabled_result = await buck.targets(
        "--config",
        "user.targets_enabled=false",
        "root//tests/targets/commands:",
    )
    assert (
        "root//tests/targets/commands:config_defined_target"
        not in targets_disabled_result.stdout
    )

    targets_cell_rel_result = await buck.targets(
        "--config",
        "upstream//user.targets_enabled=true",
        "root//tests/targets/commands:",
    )
    assert targets_cell_rel_result.stdout == targets_disabled_result.stdout


@buck_test(inplace=True)
async def test_targets_root_relative_from_fbcode(buck: Buck) -> None:
    result = await buck.targets("root//tests/targets/commands:")

    targets = [
        "root//tests/targets/commands:dynamic",
        "root//tests/targets/commands:exported",
        "root//tests/targets/commands:lib",
    ]

    for target in targets:
        assert target in result.stdout


@buck_test(inplace=True)
async def test_targets_show_output(buck: Buck) -> None:
    for target in [
        "root//tests/targets/rules/genrule:executable_helper",
        "root//tests/targets/rules/export_file:exported.txt",
    ]:
        build_result = await buck.build(target, "--show-output")
        targets_result = await buck.targets(target, "--show-output")

        build_report = build_result.get_build_report()
        build_report_outputs = [
            (target, str(output)) for output in build_report.outputs_for_target(target)
        ]
        show_output_outputs = [
            (target, os.path.join(build_report.root, output))
            for target, output in targets_result.get_target_to_build_output().items()
        ]

        assert show_output_outputs == build_report_outputs


@buck_test(inplace=True)
async def test_targets_show_output_subtargets(buck: Buck) -> None:
    TARGET = "root//tests/targets/rules/cxx:my_cpp1"
    SUBTARGET = "compilation-database"
    TARGET_WITH_SUBTARGET = (
        "root//tests/targets/rules/cxx:my_cpp1[compilation-database]"
    )

    build_result = await buck.build(
        TARGET_WITH_SUBTARGET, "--show-output", get_mode_from_platform()
    )
    targets_result = await buck.targets(
        TARGET_WITH_SUBTARGET, "--show-output", get_mode_from_platform()
    )

    build_report = build_result.get_build_report()
    build_report_outputs = [
        (TARGET_WITH_SUBTARGET, str(output))
        for output in build_report.outputs_for_target(TARGET, SUBTARGET)
    ]
    show_output_outputs = [
        (target, os.path.join(build_report.root, output))
        for target, output in targets_result.get_target_to_build_output().items()
    ]

    assert show_output_outputs == build_report_outputs


@buck_test(inplace=True)
async def test_targets_show_full_output(buck: Buck) -> None:
    for target in [
        "root//tests/targets/rules/genrule:executable_helper",
        "root//tests/targets/rules/export_file:exported.txt",
    ]:
        build_result = await buck.build(target, "--show-full-output")
        targets_result = await buck.targets(target, "--show-full-output")

        build_report = build_result.get_build_report()
        build_report_outputs = [
            (target, str(output)) for output in build_report.outputs_for_target(target)
        ]
        show_output_outputs = [
            (target, os.path.join(build_report.root, output))
            for target, output in targets_result.get_target_to_build_output().items()
        ]

        assert show_output_outputs == build_report_outputs
