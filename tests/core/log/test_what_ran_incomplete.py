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


import tempfile

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


@bsmr_test()
async def test_what_ran_incomplete(bsmr: Bsmr) -> None:
    await bsmr.build("//:my_rule")

    log = (await bsmr.log("show")).stdout.strip()
    log_file = tempfile.NamedTemporaryFile(
        suffix=".json-lines", mode="w+", delete=False
    )

    # Truncate log
    with log_file as f:
        lines = log.splitlines()
        for line in lines:
            if "SpanEnd" in line and "ActionExecution" in line:
                break
            f.write(line + "\n")
        f.close()

    target = "build\tprelude//:my_rule (<unspecified>)"

    what_ran = await bsmr.log("what-ran", "--incomplete", log_file.name)
    assert "Showing commands from:" in what_ran.stderr
    assert target in what_ran.stdout

    what_failed = await bsmr.log("what-failed", log_file.name)
    assert target not in what_failed.stdout

    what_ran = await bsmr.log("what-ran", "--show-std-err", log_file.name)
    assert "<command did not finish executing>" in what_ran.stdout
