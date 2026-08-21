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


from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


@bsmr_test(write_invocation_record=True)
async def test_representative_config_flags_disregards_run_args(
    bsmr: Bsmr,
) -> None:
    res = await bsmr.run(
        "//:my_rule",
        "--config",
        "foo.bar=baz",
        "--",
        "--config",
        "should.not=include",
    )

    assert res.invocation_record()["representative_config_flags"] == ["-c foo.bar=baz"]


@bsmr_test(write_invocation_record=True)
async def test_representative_config_flags_includes_build_args(
    bsmr: Bsmr,
) -> None:
    res = await bsmr.build(
        "--config",
        "foo.bar=baz",
        # For `build` commands, anything after `--` is a positional arg.
        "--",
        "//:my_rule",
    )

    assert res.invocation_record()["representative_config_flags"] == ["-c foo.bar=baz"]
