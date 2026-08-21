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


@bsmr_test()
async def test_target_aliases(bsmr: Bsmr) -> None:
    await bsmr.targets("alias")
    await bsmr.cquery("deps(alias)")

    await bsmr.targets("chain")
    await bsmr.cquery("deps(chain)")

    res = await bsmr.targets("--resolve-alias", "alias", "chain", "//targets:target")
    assert [line.strip() for line in res.stdout.splitlines()] == [
        "root//targets:target"
    ] * 3

    # Following a broken alias should fail
    await expect_failure(
        bsmr.targets("--resolve-alias", "bad"), stderr_regex="Invalid alias: `bad`"
    )

    # Asking for a non-existent alias / target should also fail. Note that
    # we're not capable of telling the difference between an alias that doesn't
    # exist vs. one that is broken.
    await expect_failure(
        bsmr.targets("--resolve-alias", "oops"), stderr_regex="Invalid alias: `oops`"
    )

    await expect_failure(
        bsmr.targets("--resolve-alias", "targets:not_existent"),
        stderr_regex="Invalid alias:.*Target does not exist in package",
    )
    await expect_failure(
        bsmr.targets("--resolve-alias", "broken:broken"),
        stderr_regex="Invalid alias:.*Package cannot be evaluated.*Parse error",
    )
    await expect_failure(
        bsmr.targets("--resolve-alias", "not_existent:not_existent"),
        stderr_regex="Invalid alias:.*Package cannot be evaluated.*does not exist",
    )
    await expect_failure(
        bsmr.targets("--resolve-alias", "..."),
        stderr_regex="Invalid alias.*does not expand to a single target",
    )


@bsmr_test()
async def test_resolve_alias_json(bsmr: Bsmr) -> None:
    res = await bsmr.targets(
        "--resolve-alias", "alias", "chain", "//targets:target", "--json"
    )

    assert json.loads(res.stdout) == [
        {
            "alias": "alias",
            "bsmr.package": "root//targets",
            "name": "target",
        },
        {
            "alias": "chain",
            "bsmr.package": "root//targets",
            "name": "target",
        },
        {
            "alias": "//targets:target",
            "bsmr.package": "root//targets",
            "name": "target",
        },
    ]


@bsmr_test()
async def test_resolve_alias_json_lines(bsmr: Bsmr) -> None:
    res = await bsmr.targets(
        "--resolve-alias", "alias", "chain", "//targets:target", "--json-lines"
    )

    lines = [line.strip() for line in res.stdout.splitlines()]
    lines = [line for line in lines if line]

    assert [json.loads(line) for line in res.stdout.splitlines()] == [
        {
            "alias": "alias",
            "bsmr.package": "root//targets",
            "name": "target",
        },
        {
            "alias": "chain",
            "bsmr.package": "root//targets",
            "name": "target",
        },
        {
            "alias": "//targets:target",
            "bsmr.package": "root//targets",
            "name": "target",
        },
    ]
