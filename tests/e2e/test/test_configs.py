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
import tempfile
from typing import Final

from bsmr.tests.e2e_util.api.buck import Buck
from bsmr.tests.e2e_util.buck_workspace import buck_test

FBCODE_TARGET: Final[str] = "upstream//testinfra/playground/cpp/tests:test_example"
ARVR_TARGET: Final[str] = (
    "upstream//arvr/projects/tcc_playground/python_unittest:test_example"
)


@buck_test(inplace=True)
async def test_no_args(buck: Buck) -> None:
    bsmr_config = await execute_test_with_args(buck, [], target=FBCODE_TARGET)

    assert_buck_args_config_equal(
        bsmr_config,
        {
            "mode": "@upstream//mode/dev",
            "config": "",
            "host": "linux",
        },
    )

    bsmr_config = await execute_test_with_args(
        buck,
        [],
        target=ARVR_TARGET,
    )

    assert_buck_args_config_equal(
        bsmr_config,
        {
            "mode": "",
            "config": "",
            "host": "linux",
        },
    )


@buck_test(inplace=True)
async def test_mode_file(buck: Buck) -> None:
    all_configs_to_test = [
        ["@upstream//mode/dev"],
        ["--flagfile", "upstream//mode/dev"],
    ]
    for config in all_configs_to_test:
        bsmr_config = await execute_test_with_args(buck, config, target=FBCODE_TARGET)
        assert_buck_args_config_equal(
            bsmr_config,
            {
                "mode": "@upstream//mode/dev",
                "config": "",
                "host": "linux",
            },
        )


@buck_test(inplace=True)
async def test_mode_file_non_default(buck: Buck) -> None:
    all_configs_to_test = [
        ["@upstream//mode/opt"],
        ["--flagfile", "upstream//mode/opt"],
    ]
    for config in all_configs_to_test:
        bsmr_config = await execute_test_with_args(buck, config, target=FBCODE_TARGET)
        assert_buck_args_config_equal(
            bsmr_config,
            {
                "mode": "@upstream//mode/opt",
                "config": "",
                "host": "linux",
            },
        )


@buck_test(inplace=True)
async def test_multi_mode_file(buck: Buck) -> None:
    all_configs_to_test = [
        ["@upstream//mode/opt", "@upstream//mode/dev"],
        ["--flagfile", "upstream//mode/opt", "--flagfile", "upstream//mode/dev"],
        ["--flagfile", "upstream//mode/opt", "@upstream//mode/dev"],
    ]
    for config in all_configs_to_test:
        bsmr_config = await execute_test_with_args(buck, config, target=FBCODE_TARGET)
        assert_buck_args_config_equal(
            bsmr_config,
            {
                "mode": "@upstream//mode/opt;@upstream//mode/dev",
                "config": "",
                "host": "linux",
            },
        )


@buck_test(inplace=True)
async def test_multi_mode_file_deduplication(buck: Buck) -> None:
    all_configs_to_test = [
        ["@upstream//mode/opt", "@upstream//mode/dev", "@upstream//mode/dev"],
        ["@upstream//mode/opt", "@upstream//mode/opt", "@upstream//mode/dev"],
        ["@upstream//mode/dev", "@upstream//mode/opt", "@upstream//mode/dev"],
    ]
    for config in all_configs_to_test:
        bsmr_config = await execute_test_with_args(buck, config, target=FBCODE_TARGET)
        assert_buck_args_config_equal(
            bsmr_config,
            {
                "mode": "@upstream//mode/opt;@upstream//mode/dev",
                "config": "",
                "host": "linux",
            },
        )


@buck_test(inplace=True)
async def test_config(buck: Buck) -> None:
    # certain config makes it to the buck config
    all_configs_to_test = [
        ["--config", "fbcode.use_link_groups_in_dev=True"],
        ["--config=fbcode.use_link_groups_in_dev=True"],
        ["-cfbcode.use_link_groups_in_dev=True"],
        ["-c", "fbcode.use_link_groups_in_dev=True"],
    ]
    for config in all_configs_to_test:
        bsmr_config = await execute_test_with_args(buck, config, target=FBCODE_TARGET)
        assert_buck_args_config_equal(
            bsmr_config,
            {
                "mode": "@upstream//mode/dev",
                "config": "fbcode.use_link_groups_in_dev=True",
                "host": "linux",
            },
        )

    # some configs are dropped
    bsmr_config = await execute_test_with_args(
        buck, ["-c", "bsmr.log_configured_graph_size=true"], target=FBCODE_TARGET
    )
    assert_buck_args_config_equal(
        bsmr_config,
        {
            "mode": "@upstream//mode/dev",
            "config": "",
            "host": "linux",
        },
    )


@buck_test(inplace=True)
async def test_config_deduplication(buck: Buck) -> None:
    # certain config makes it to the buck config
    all_configs_to_test = [
        [
            "--config=fbcode.use_link_groups_in_dev=True",
            "--config=fbcode.split-dwarf=false",
        ],
        [
            "--config=fbcode.use_link_groups_in_dev=True",
            "--config=fbcode.split-dwarf=true",
            "--config=fbcode.split-dwarf=false",
        ],
        [
            "--config=fbcode.use_link_groups_in_dev=False",
            "--config=fbcode.use_link_groups_in_dev=True",
            "--config=fbcode.split-dwarf=false",
        ],
        [
            "--config=fbcode.use_link_groups_in_dev=False",
            "--config=fbcode.use_link_groups_in_dev=True",
            "--config=fbcode.split-dwarf=true",
            "--config=fbcode.split-dwarf=false",
        ],
    ]
    for config in all_configs_to_test:
        bsmr_config = await execute_test_with_args(buck, config, target=FBCODE_TARGET)
        assert_buck_args_config_equal(
            bsmr_config,
            {
                "mode": "@upstream//mode/dev",
                "config": "fbcode.split-dwarf=false;fbcode.use_link_groups_in_dev=True",
                "host": "linux",
            },
        )


@buck_test(inplace=True)
async def test_modifier(buck: Buck) -> None:
    all_configs_to_test = [
        ["--modifier", "dev"],
        ["--modifier=dev"],
        ["-m", "dev"],
        ["-mdev"],
    ]
    for config in all_configs_to_test:
        bsmr_config = await execute_test_with_args(buck, config, target=FBCODE_TARGET)
        assert_buck_args_config_equal(
            bsmr_config,
            {
                "mode": "@upstream//mode/dev",
                "config": "",
                "host": "linux",
                "modifier": "dev",
            },
        )


@buck_test(inplace=True)
async def test_modifier_deduplication(buck: Buck) -> None:
    all_configs_to_test = [
        ["-m", "dev", "-m", "opt", "-m", "dev"],
    ]
    for config in all_configs_to_test:
        bsmr_config = await execute_test_with_args(buck, config, target=FBCODE_TARGET)
        assert_buck_args_config_equal(
            bsmr_config,
            {
                "mode": "@upstream//mode/dev",
                "config": "",
                "host": "linux",
                "modifier": "opt;dev",
            },
        )


#########
# Helpers
#########


# run the buck test command and return the actual used buck config
async def execute_test_with_args(
    buck: Buck,
    args: list[str],
    target: str,
) -> dict[str, str]:
    tpx_trace_path = tempfile.NamedTemporaryFile(delete=False)
    await buck.test(
        *args,
        target,
        "--",
        "--trace-file-path",
        tpx_trace_path.name,
    )

    return get_bsmr_config(tpx_trace_path.name)


def get_bsmr_config(tpx_trace_path: str) -> dict[str, str]:
    # Each row in the log is a json string
    # Find the first one with 'event name == run.external_bsmr_config_finalized'
    with open(tpx_trace_path, encoding="utf-8") as f:
        for _, line in enumerate(f):
            data = json.loads(line)
            if "fields" in data and "event_name" in data["fields"]:
                if data["fields"]["event_name"] == "utf.process_selector.include":
                    return dict(json.loads(data["fields"]["external_config"]))
    return {}


def assert_buck_args_config_equal(
    actual_config: dict[str, str], expected_config: dict[str, str]
) -> None:
    assert actual_config == expected_config, (
        f"Expected {expected_config}, got {actual_config}"
    )
