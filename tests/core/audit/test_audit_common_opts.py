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


import pytest
from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test

# TODO(iguridi) or TODO(raulgarcia4):
# New `audit` commands have been added since these tests were created.
# Test them if necessary.


@bsmr_test()
@pytest.mark.parametrize(  # type: ignore
    "cmd",
    [
        "audit_visibility",
        "audit_configurations",
        "audit_config",
        "audit_visibility",
    ],
)
async def test_pass_common_opts_func(bsmr: Bsmr, cmd: str) -> None:
    cmd_call = getattr(bsmr, cmd)
    await cmd_call("--client-metadata", "id=placeholder_id")


@bsmr_test()
@pytest.mark.parametrize(  # type: ignore
    "cmd",
    [
        "analysis-queries",
        "cell",
        "execution-platform-resolution",
        "includes",
        "prelude",
        "providers",
        "subtargets",
    ],
)
async def test_pass_common_opts(bsmr: Bsmr, cmd: str) -> None:
    commands_requiring_target_pattern_arg_value = {"providers", "subtargets"}

    if cmd in commands_requiring_target_pattern_arg_value:
        await bsmr.audit(cmd, "//:dummy", "--client-metadata", "id=placeholder_id")
    else:
        await bsmr.audit(cmd, "--client-metadata", "id=placeholder_id")
