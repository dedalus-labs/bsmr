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


@bsmr_test(inplace=True)
async def test_set_cfg_modifiers(bsmr: Bsmr) -> None:
    result = await bsmr.targets(
        "root//tests/e2e/configurations/cfg_constructor/test_clear_package_modifiers_data/set_cfg_modifiers/dir:test",
        "--package-values",
    )
    targets = json.loads(result.stdout)
    assert len(targets) == 1
    target = targets[0]
    cfg_modifiers = target["bsmr.package_values"]["bsmr.cfg_modifiers"]
    assert cfg_modifiers == [
        {
            "_type": "TaggedModifiers",
            "location": {
                "_type": "ModifierPackageLocation",
                "package_path": "root//tests/e2e/configurations/cfg_constructor/test_clear_package_modifiers_data/set_cfg_modifiers/PACKAGE",
            },
            "modifiers": [
                {
                    "_type": "ModifiersMatch",
                    "ovr_config//os/constraints:linux": "ovr_config//cpu/constraints:arm64",
                    "ovr_config//os/constraints:macos": "ovr_config//cpu/constraints:x86_64",
                },
                {
                    "DEFAULT": "ovr_config//os/constraints:linux",
                    "_type": "ModifiersMatch",
                },
            ],
            "rule_name": None,
        },
        {
            "_type": "TaggedModifiers",
            "location": {
                "_type": "ModifierPackageLocation",
                "package_path": "root//tests/e2e/configurations/cfg_constructor/test_clear_package_modifiers_data/set_cfg_modifiers/PACKAGE",
            },
            "modifiers": [
                "ovr_config//cpu/constraints:x86_64",
            ],
            "rule_name": "python_binary",
        },
        {
            "_type": "TaggedModifiers",
            "location": {
                "_type": "ModifierPackageLocation",
                "package_path": "root//tests/e2e/configurations/cfg_constructor/test_clear_package_modifiers_data/set_cfg_modifiers/dir/PACKAGE",
            },
            "modifiers": [
                {
                    "_type": "ModifiersMatch",
                    "ovr_config//os/constraints:windows": "ovr_config//cpu/constraints:x86_64",
                },
                "ovr_config//os/constraints:macos",
            ],
            "rule_name": None,
        },
    ]


@bsmr_test(inplace=True)
async def test_set_cfg_modifiers_from_package_file_only(
    bsmr: Bsmr,
) -> None:
    await expect_failure(
        bsmr.targets(
            "root//tests/e2e/configurations/cfg_constructor/test_clear_package_modifiers_data/set_cfg_modifiers/package_file_check:test",
            "-c",
            "bsmr_e2e.testing_failure=true",
        ),
        stderr_regex="set_cfg_modifiers is only allowed to be used from a PACKAGE or BSMR_TREE file, not a bzl file",
    )
