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
async def test_build_universe(bsmr: Bsmr) -> None:
    # Run the build without universe.
    result = await bsmr.build("//:test")
    build_report = result.get_build_report()
    output = build_report.output_for_target("//:test")
    assert output.read_text().rstrip() == "default"

    # Now build the same target, but with the universe.
    result = await bsmr.build(
        "//:test",
        "--target-universe",
        "//:universe",
    )
    build_report = result.get_build_report()
    output = build_report.output_for_target("//:test")
    assert output.read_text().rstrip() == "cat"


@bsmr_test()
async def test_build_target_not_found_in_universe(bsmr: Bsmr) -> None:
    result = await bsmr.build(
        "//:test",
        "--target-universe",
        "//:different_universe",
    )

    assert (
        "No targets found inside the specified universe, nothing will be built"
        in result.stderr
    )
