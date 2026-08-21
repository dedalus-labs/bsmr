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


@bsmr_test()
async def test_bxl_audit_output(bsmr: Bsmr) -> None:
    await bsmr.bxl(
        "//audit.bxl:audit_output_action_exists",
    )

    await bsmr.bxl(
        "//audit.bxl:audit_output_config_not_match",
    )

    await expect_failure(
        bsmr.bxl(
            "//audit.bxl:audit_output_invalid_path",
        ),
        stderr_regex="Malformed bsmr-out path",
    )


@bsmr_test()
async def test_bxl_audit_content_based_output(bsmr: Bsmr) -> None:
    label = "root//:with_content_based_output"
    result = await bsmr.build(label, "--show-output")
    path = result.get_build_report().output_for_target(label)

    # resolve the symlink that we get as the output from bsmr to find the underlying content-based path.
    path = (bsmr.cwd / path).resolve()
    # make it a relative path again
    path = path.relative_to(bsmr.cwd)

    await bsmr.bxl(
        "//audit.bxl:audit_content_based_output_action_exists",
        "--",
        "--label",
        label,
        "--path",
        path.as_posix(),
    )
