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


from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


@bsmr_test()
async def test_bxl_caching(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//caching.bxl:print_caching",
    )

    assert "ran me" in result.stderr
    assert "result print" in result.stdout

    result = await bsmr.bxl(
        "//caching.bxl:print_caching",
    )

    assert "ran me" not in result.stderr
    assert "result print" in result.stdout


@bsmr_test()
async def test_bxl_caching_with_target_platforms_specified(bsmr: Bsmr) -> None:
    # run with platform1, result should be cached afterwards
    result = await bsmr.bxl(
        "//caching.bxl:caching_with_target_platforms",
        "--target-platforms",
        "root//:platform1",
    )

    assert "ran me" in result.stderr
    assert "root//:platform1" in result.stdout

    # run with platform2, DICE should be invalidated and updated results should be
    # cached afterwards
    result = await bsmr.bxl(
        "//caching.bxl:caching_with_target_platforms",
        "--target-platforms",
        "root//:platform2",
    )

    assert "ran me" in result.stderr
    assert "root//:platform2" in result.stdout

    # run with platform1 again, we should already have cached results
    result = await bsmr.bxl(
        "//caching.bxl:caching_with_target_platforms",
        "--target-platforms",
        "root//:platform1",
    )

    assert "ran me" not in result.stderr
    assert "root//:platform1" in result.stdout


@bsmr_test()
async def test_bxl_error_caching(bsmr: Bsmr) -> None:
    result = await bsmr.bxl("//caching.bxl:print_error_caching")
    assert "ran me" in result.stderr
    assert "Skipped 1 incompatible targets" in result.stderr
    assert "root//:incompatible" in result.stderr

    # output stream that writes to stderr should be cached, but regular stdlib print
    # statements (which also write to stderr) will not be cached.
    result = await bsmr.bxl("//caching.bxl:print_error_caching")
    assert "ran me" not in result.stderr
    assert "Skipped 1 incompatible targets" in result.stderr
    assert "root//:incompatible" in result.stderr


@bsmr_test()
async def test_bxl_print_with_no_daemon(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//caching.bxl:print_caching",
        "--no-bsmrd",
    )

    assert "ran me" in result.stderr
    assert "result print" in result.stdout
