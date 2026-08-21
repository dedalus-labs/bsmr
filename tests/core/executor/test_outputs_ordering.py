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

import os
import re
import subprocess

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test
from bsmr.tests.e2e_util.helper.utils import (
    get_bsmr_re_use_case,
    json_get,
    random_string,
    read_what_ran,
)


@bsmr_test()
async def test_local_action(bsmr: Bsmr) -> None:
    await bsmr.build(
        "//:foo",
        "--no-remote-cache",
        "--local-only",
        "-c",
        f"test.cache_buster={random_string()}",
    )

    log = (await bsmr.log("show")).stdout.strip().splitlines()

    for line in log:
        outputs = json_get(
            line,
            "Event",
            "data",
            "SpanEnd",
            "data",
            "ActionExecution",
            "outputs",
        )
        if outputs is None:
            continue
        # da39a3ee is a digest for empty directory.
        # We have 2 directories "a" and "z", where
        # "a" is empty and "z" is not.
        # "z" is a first output for action.
        digests = [o["tiny_digest"] for o in outputs]
        assert len(digests) == 2
        # Checking that "a" is first in action outputs
        assert digests[0] == "da39a3ee"
        return

    raise AssertionError("Didn't find ActionExecution data")


@bsmr_test()
async def test_remote_action(bsmr: Bsmr) -> None:
    await bsmr.build(
        "//:foo",
        "--no-remote-cache",
        "--remote-only",
        "-c",
        f"test.cache_buster={random_string()}",
    )

    log = (await bsmr.log("show")).stdout.strip().splitlines()

    for line in log:
        outputs = json_get(
            line,
            "Event",
            "data",
            "SpanEnd",
            "data",
            "ActionExecution",
            "outputs",
        )
        if outputs is None:
            continue
        # da39a3ee is a digest for empty directory.
        # We have 2 directories "a" and "z", where
        # "a" is empty and "z" is not.
        # "z" is a first output for action.
        digests = [o["tiny_digest"] for o in outputs]
        assert len(digests) == 2
        # Checking that "a" is first in action outputs
        assert digests[0] == "da39a3ee"
        break

    what_ran = await read_what_ran(bsmr)
    assert len(what_ran) == 1
    digest = what_ran[0]["reproducer"]["details"]["digest"]
    use_case = await get_bsmr_re_use_case(bsmr)
    action_definition = subprocess.check_output(
        [
            "dotslash",
            os.environ["RECLI"],
            "--use-case",
            use_case,
            "cas",
            "download-action",
            digest,
        ],
        text=True,
    )
    # Though RE action has "a" first and then "z"
    assert (
        re.search(
            'Output\\WDirectories.+\n\\["bsmr-out/.+/__foo__/a",\\W"bsmr-out/.+/__foo__/z"',
            action_definition,
        )
        is not None
    )
