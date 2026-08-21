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
async def test_keep_going_json(bsmr: Bsmr) -> None:
    result = await bsmr.targets("//...", "--json", "--keep-going")
    xs = json.loads(result.stdout)
    # I expect six records, one which is an error
    assert len(xs) == 6
    for x in xs:
        if x["bsmr.package"] == "root//a":
            assert x["name"].startswith("target")
        else:
            assert x["bsmr.package"] == "root//b"
            assert "test_error" in x["bsmr.error"]


@bsmr_test()
async def test_keep_going(bsmr: Bsmr) -> None:
    result = await bsmr.targets("//...", "--keep-going")
    assert "test_error" in result.stderr


@bsmr_test()
async def test_keep_going_streaming(bsmr: Bsmr) -> None:
    result = await bsmr.targets("//...", "--streaming", "--keep-going")
    assert "test_error" in result.stderr


@bsmr_test()
async def test_streaming_keep_going_missing_targets(bsmr: Bsmr) -> None:
    targets = [
        "//a:target1",
        "//a:target2",
        "//a:bogus_target",
        "//a:worse_target",
        "//a:target5",
        "//d:bogus_package",
    ]
    result = await bsmr.targets(*targets, "--json", "--streaming", "--keep-going")
    xs = json.loads(result.stdout)
    assert len(xs) == 5  # 3 success, 2 errors
    bad_packages = []
    good_targets = []
    for x in xs:
        if "bsmr.error" in x:
            bad_packages.append(x["bsmr.package"])
            if x["bsmr.package"] == "root//a":
                assert "`bogus_target`" in x["bsmr.error"]
                assert "`worse_target`" in x["bsmr.error"]
        else:
            good_targets.append(x["name"])
    bad_packages.sort()
    good_targets.sort()
    assert bad_packages == ["root//a", "root//d"]
    assert good_targets == ["target1", "target2", "target5"]


@bsmr_test()
async def test_streaming_keep_going_with_single_failure(bsmr: Bsmr) -> None:
    targets = [
        "//a:does_not_exist",
    ]
    result = await bsmr.targets(*targets, "--json", "--streaming", "--keep-going")
    xs = json.loads(result.stdout)
    assert len(xs) == 1
    assert xs[0]["bsmr.package"] == "root//a"
    assert (
        xs[0]["bsmr.error"]
        == "Unknown targets `does_not_exist` from package `root//a`."
    )


@bsmr_test()
async def test_streaming_keep_going_with_single_failing_target_and_one_other_target_in_different_package(
    bsmr: Bsmr,
) -> None:
    targets = [
        "//a:target1",
        "//c:does_not_exist",
    ]
    result = await bsmr.targets(
        *targets,
        "-a",
        "type",
        "--streaming",
        "--keep-going",
    )

    xs = json.loads(result.stdout)
    assert len(xs) == 2

    if "bsmr.error" in xs[0]:
        good_target = xs[1]
        bad_target = xs[0]
    else:
        good_target = xs[0]
        bad_target = xs[1]

    assert good_target["bsmr.type"] == "prelude//prelude.bzl:a_target"

    assert bad_target["bsmr.package"] == "root//c"
    assert (
        bad_target["bsmr.error"]
        == "Unknown targets `does_not_exist` from package `root//c`."
    )
