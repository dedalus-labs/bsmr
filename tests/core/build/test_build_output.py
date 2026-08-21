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
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test
from bsmr.tests.e2e_util.helper.golden import golden


@bsmr_test()
async def test_build_output(bsmr: Bsmr) -> None:
    show_output = await bsmr.build_without_report(
        "root//:foo",
        "--show-output",
    )
    show_full_output = await bsmr.build_without_report(
        "root//:foo",
        "--show-full-output",
    )
    show_simple_output = await bsmr.build_without_report(
        "root//:foo",
        "--show-simple-output",
    )
    show_json_output = await bsmr.build_without_report(
        "root//:foo",
        "--show-json-output",
    )

    output = "\n\n".join(
        [
            show_output.stdout,
            show_full_output.stdout,
            show_simple_output.stdout,
            show_json_output.stdout,
        ]
    )
    output = output.replace(str(bsmr.cwd), "/abs/project/root")
    output = output.replace("\\\\", "/")  # Windows path separators in json
    output = output.replace("\\", "/")  # Windows path separators not in json

    golden(
        output=output,
        rel_path="build_output.golden",
    )


@bsmr_test()
async def test_build_output_on_partial_success(bsmr: Bsmr) -> None:
    show_output = await expect_failure(
        bsmr.build_without_report(
            "root//:foo",
            "root//:fail",
            "--show-simple-output",
        )
    )
    assert len(show_output.stdout.splitlines()) == 1
