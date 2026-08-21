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
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


def _replace_hash(s: str) -> str:
    return re.sub(r"\b[0-9a-f]{16}\b", "<HASH>", s)


@bsmr_test()
async def test_cquery_transition_without_target_universe(bsmr: Bsmr) -> None:
    result = await bsmr.cquery(
        "root//:bsmr",
        "--target-platforms=root//:p",
    )

    # Both configurations for the target are returned: the default, and the transition
    lines = result.stdout.splitlines()
    assert 2 == len(lines)
    assert _replace_hash(lines[0]) == "root//:bsmr (root//:p#<HASH>)"
    assert _replace_hash(lines[1]) == "root//:bsmr (transitioned-to-reindeer#<HASH>)"

    # Test cquery with "%s".
    result = await bsmr.cquery(
        "%s",
        "root//:bsmr",
        "root//:moose",
        "--target-platforms=root//:p",
    )

    lines = result.stdout.splitlines()
    assert 4 == len(lines)
    assert _replace_hash(lines[0]) == "root//:bsmr (root//:p#<HASH>)"
    assert _replace_hash(lines[1]) == "root//:bsmr (transitioned-to-reindeer#<HASH>)"
    assert _replace_hash(lines[2]) == "root//:moose (root//:p#<HASH>)"
    assert _replace_hash(lines[3]) == "root//:moose (transitioned-to-reindeer#<HASH>)"

    # Test cquery with "%Ss"
    result = await bsmr.cquery(
        "%Ss",
        "root//:bsmr",
        "root//:moose",
        "--target-platforms=root//:p",
    )

    lines = result.stdout.splitlines()
    assert 4 == len(lines)
    assert _replace_hash(lines[0]) == "root//:bsmr (root//:p#<HASH>)"
    assert _replace_hash(lines[1]) == "root//:bsmr (transitioned-to-reindeer#<HASH>)"
    assert _replace_hash(lines[2]) == "root//:moose (root//:p#<HASH>)"
    assert _replace_hash(lines[3]) == "root//:moose (transitioned-to-reindeer#<HASH>)"


@bsmr_test()
async def test_cquery_transition_with_target_universe(bsmr: Bsmr) -> None:
    result = await bsmr.cquery(
        "root//:bsmr",
        "--target-platforms=root//:p",
        "--target-universe",
        "root//:bsmr",
    )

    lines = result.stdout.splitlines()
    assert 2 == len(lines)
    assert _replace_hash(lines[0]) == "root//:bsmr (root//:p#<HASH>)"
    assert _replace_hash(lines[1]) == "root//:bsmr (transitioned-to-reindeer#<HASH>)"

    # Test cquery with "%s".
    result = await bsmr.cquery(
        "%s",
        "root//:bsmr",
        "root//:moose",
        "--target-platforms=root//:p",
        "--target-universe",
        "root//:bsmr,root//:moose",
    )

    lines = result.stdout.splitlines()
    assert 4 == len(lines)
    assert _replace_hash(lines[0]) == "root//:bsmr (root//:p#<HASH>)"
    assert _replace_hash(lines[1]) == "root//:bsmr (transitioned-to-reindeer#<HASH>)"
    assert _replace_hash(lines[2]) == "root//:moose (root//:p#<HASH>)"
    assert _replace_hash(lines[3]) == "root//:moose (transitioned-to-reindeer#<HASH>)"

    # Test cquery with "%Ss".
    result = await bsmr.cquery(
        "%Ss",
        "root//:bsmr",
        "root//:moose",
        "--target-platforms=root//:p",
        "--target-universe",
        "root//:bsmr,root//:moose",
    )

    lines = result.stdout.splitlines()
    assert 4 == len(lines)
    assert _replace_hash(lines[0]) == "root//:bsmr (root//:p#<HASH>)"
    assert _replace_hash(lines[1]) == "root//:bsmr (transitioned-to-reindeer#<HASH>)"
    assert _replace_hash(lines[2]) == "root//:moose (root//:p#<HASH>)"
    assert _replace_hash(lines[3]) == "root//:moose (transitioned-to-reindeer#<HASH>)"
