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
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


@bsmr_test()
async def test_unconfigured_target_hashing(
    bsmr: Bsmr,
) -> None:
    await assert_hashes(bsmr, ":foo", "foo.txt", False)
    await assert_hashes(bsmr, ":foo", "bar.txt", True)
    await assert_hashes(bsmr, ":foo_dep", "foo.txt", False)
    await assert_hashes(bsmr, ":foo_dep", "bar.txt", True)
    await assert_hashes(bsmr, ":none", "bar.txt", True)


async def assert_hashes(
    bsmr: Bsmr, target: str, modified_path: str, same_hash: bool
) -> None:
    result = await bsmr.targets(
        target,
        "--show-unconfigured-target-hash",
        "--json",
        "--target-hash-file-mode",
        "PATHS_ONLY",
        "--target-hash-recursive=true",
    )

    modified_result = await bsmr.targets(
        target,
        "--show-unconfigured-target-hash",
        "--json",
        "--target-hash-file-mode",
        "PATHS_ONLY",
        "--target-hash-recursive=true",
        "--target-hash-modified-paths",
        modified_path,
    )
    output = json.loads(result.stdout)
    modified_output = json.loads(modified_result.stdout)

    # Hash should change if modified path belongs to target or to any of its dependencies
    if same_hash:
        assert output[0]["bsmr.target_hash"] == modified_output[0]["bsmr.target_hash"]
    else:
        assert output[0]["bsmr.target_hash"] != modified_output[0]["bsmr.target_hash"]


@bsmr_test()
async def test_cfg_modifiers_change_target_hash(bsmr: Bsmr) -> None:
    result = await bsmr.targets(
        ":foo",
        "--show-unconfigured-target-hash",
        "--target-hash-recursive=false",
        "--json",
    )

    with open(bsmr.cwd / "PACKAGE", "w") as package:
        package.write("set_modifiers(['aaabbbccc'])")

    modified_result = await bsmr.targets(
        ":foo",
        "--show-unconfigured-target-hash",
        "--target-hash-recursive=false",
        "--json",
    )
    output = json.loads(result.stdout)
    modified_output = json.loads(modified_result.stdout)

    # modifiers should change target hash
    assert output[0]["bsmr.target_hash"] != modified_output[0]["bsmr.target_hash"]


@bsmr_test()
async def test_parent_cfg_modifiers_change_target_hash(bsmr: Bsmr) -> None:
    result = await bsmr.targets(
        "foo:bar",
        "--show-unconfigured-target-hash",
        "--target-hash-recursive=false",
        "--json",
    )

    with open(bsmr.cwd / "PACKAGE", "w") as package:
        package.write("set_modifiers(['aaabbbccc'])")

    modified_result = await bsmr.targets(
        "foo:bar",
        "--show-unconfigured-target-hash",
        "--target-hash-recursive=false",
        "--json",
    )
    output = json.loads(result.stdout)
    modified_output = json.loads(modified_result.stdout)

    # parent set_modifiers value should change target hash
    # note that we merge parent modifiers and current package modifiers
    assert output[0]["bsmr.target_hash"] != modified_output[0]["bsmr.target_hash"]
