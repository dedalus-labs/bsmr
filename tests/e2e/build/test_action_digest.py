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

from pathlib import Path

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test, get_mode_from_platform
from bsmr.tests.e2e_util.helper.utils import read_what_ran


# If this test fails, it means that a change that modifies action digest was made.
# Background in this post:
# https://fb.workplace.com/groups/bsmreng/permalink/3452581371706005/
# Changes should instead be deployed by:
#   1: Create a new bsmr flag and hide the changes behind it (Ex. D59503359)
#   2: Wait for bvb that contains #1 to land. To be safe, wait for a second to land
#      as well so you're guaranteed that the first bump can no longer be
#      fast-reverted.
#   3: Activate the flag via .bsmr (Ex. D59648609)
#       3.1: Fix/followup on any CI failures caused by cache invalidation
#   4: Observe for a couple of days to ensure that there are no issues
#   5. Remove the code associated with the config flag but NOT the config itself,
#      this way this test wouldn't need to be changed at all (Ex. D59864942)
#   6: Wait for bvb that contains #5 to land. Optionally wait for a second as above.
#   7: Remove the config flag (Ex. D59988979)
@bsmr_test(inplace=True)
async def test_action_digest(bsmr: Bsmr) -> None:
    await bsmr.build(
        get_mode_from_platform(),
        "root//tests/targets/rules/rust/hello_world:welcome",
        "--remote-only",
    )
    compiled_out = await read_what_ran(bsmr)
    compiled_digests = [
        entry["reproducer"]["details"]["digest"] for entry in compiled_out
    ]
    compiled_digests.sort()

    # TODO(nga): this should also test reverted bsmr.
    bsmr.path_to_executable = Path("bsmr")
    await bsmr.build(
        get_mode_from_platform(),
        "root//tests/targets/rules/rust/hello_world:welcome",
        "--remote-only",
    )
    deployed_out = await read_what_ran(bsmr)
    deployed_digests = [
        entry["reproducer"]["details"]["digest"] for entry in deployed_out
    ]
    deployed_digests.sort()

    assert compiled_digests == deployed_digests, (
        "Action Digest was modified, refer to comment on this test for next steps"
    )
