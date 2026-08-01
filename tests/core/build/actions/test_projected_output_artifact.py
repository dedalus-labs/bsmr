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


from bsmr.tests.e2e_util.api.buck import Buck
from bsmr.tests.e2e_util.buck_workspace import buck_test


@buck_test()
async def test_projected_output_artifact_write(buck: Buck) -> None:
    res = await buck.build("root//:write")
    # TODO(nga): this is a bug: we write into projected artifact, but return original artifact,
    #   and yet here we read from original non-projected artifact.
    assert (
        "ccoonntteenntt"
        == res.get_build_report().output_for_target("root//:write").read_text()
    )


@buck_test()
async def test_projected_output_artifact_run(buck: Buck) -> None:
    res = await buck.build("root//:run")
    assert (
        "hello"
        == (res.get_build_report().output_for_target("root//:run") / "rel").read_text()
    )
