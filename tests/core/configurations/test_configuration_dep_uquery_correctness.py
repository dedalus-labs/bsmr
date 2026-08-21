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


async def check_has_uquery_path(
    bsmr: Bsmr, target: str, dep: str, expect_fail: bool = False
) -> None:
    result = await bsmr.uquery(
        f"somepath({target}, {dep})",
    )
    path = result.stdout.splitlines()
    # Apparently, configuration deps never show up in `somepath`. Interesting.
    assert len(path) == 0

    result = await bsmr.uquery(
        f"deps({target})",
        "-a",
        "bsmr.deps",
        "-a",
        "bsmr.configuration_deps",
    )
    all_deps = [
        d
        for node in json.loads(result.stdout).values()
        for deps in node.values()
        for d in deps
    ]
    if expect_fail:
        assert dep not in all_deps
    else:
        assert dep in all_deps


@bsmr_test()
async def test_default_target_platform(bsmr: Bsmr) -> None:
    await check_has_uquery_path(bsmr, ":with_custom_dtp", "root//:base")


@bsmr_test()
async def test_configured_dep_platform(bsmr: Bsmr) -> None:
    await check_has_uquery_path(bsmr, ":stub_configured", "root//:base")


@bsmr_test()
async def test_transition_dep_refs(bsmr: Bsmr) -> None:
    # FIXME(JakobDegen): Bug.
    await check_has_uquery_path(
        bsmr, ":pre_out_transition", "root//:cat", expect_fail=True
    )

    # FIXME(JakobDegen): Bug.
    await check_has_uquery_path(
        bsmr, ":post_out_transition", "root//:cat", expect_fail=True
    )

    await check_has_uquery_path(bsmr, ":pre_out_transition_vnew", "root//:transition")

    await check_has_uquery_path(bsmr, ":pre_inc_transition_vnew", "root//:transition")


@bsmr_test()
async def test_select_keys(bsmr: Bsmr) -> None:
    await check_has_uquery_path(bsmr, ":with_select", "root//:cat")
