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
from bsmr.tests.e2e_util.helper.utils import filter_events


@bsmr_test()
async def test_forward_node_supports_cutoff(bsmr: Bsmr) -> None:
    await bsmr.targets("--show-output", "root//:main")
    # Add a file to the root directory
    with open(bsmr.cwd / "TARGETS.fixture", "a") as targetsfile:
        targetsfile.write("\n# a comment\n")
    await bsmr.targets("--show-output", "root//:main")

    events = await filter_events(bsmr, "Event", "data", "SpanEnd", "data")
    loads = []
    analyses = []

    for ev in events:
        if "Load" in ev:
            loads.append(ev)
        if "Analysis" in ev:
            analyses.append(ev)

    assert len(loads) > 0
    # TODO(cjhopman): fix
    assert len(analyses) == 0, "should not have analysed anything"
