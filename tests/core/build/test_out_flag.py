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
import tempfile
from pathlib import Path

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


@bsmr_test()
async def test_out_single_default_output(bsmr: Bsmr) -> None:
    with tempfile.TemporaryDirectory() as out:
        output = os.path.join(out, "output")
        await bsmr.build("//:a", "--out", output)
        with open(output) as readable:
            assert readable.read() == "a\n"


@bsmr_test()
async def test_out_overwrite(bsmr: Bsmr) -> None:
    with tempfile.TemporaryDirectory() as out:
        output = os.path.join(out, "output")
        await bsmr.build("//:a", "--out", output)
        await bsmr.build("//:a", "--out", output)


@bsmr_test()
async def test_out_parent_not_exist(bsmr: Bsmr) -> None:
    with tempfile.TemporaryDirectory() as out:
        output = os.path.join(out, "notexist", "output")
        await bsmr.build("//:a", "--out", output)
        with open(output) as readable:
            assert readable.read() == "a\n"


@bsmr_test()
async def test_out_single_default_output_to_dir(bsmr: Bsmr) -> None:
    with tempfile.TemporaryDirectory() as out:
        await bsmr.build("//:a", "--out", out)
        with open(Path(out) / "a.txt") as readable:
            assert readable.read() == "a\n"


@bsmr_test()
async def test_out_no_outputs(bsmr: Bsmr) -> None:
    with tempfile.NamedTemporaryFile("w") as out:
        await expect_failure(
            bsmr.build("//:none", "--out", out.name),
            stderr_regex="produced zero default outputs",
        )


@bsmr_test()
async def test_out_multiple_outputs(bsmr: Bsmr) -> None:
    with tempfile.NamedTemporaryFile("w") as out:
        await expect_failure(
            bsmr.build("//:ab", "--out", out.name),
            stderr_regex="produced 2 outputs",
        )


@bsmr_test()
async def test_out_multiple_targets(bsmr: Bsmr) -> None:
    with tempfile.NamedTemporaryFile("w") as out:
        await expect_failure(
            bsmr.build("//:a", "//:b", "--out", out.name),
            stderr_regex="command built multiple top-level targets",
        )


@bsmr_test()
async def test_out_directory(bsmr: Bsmr) -> None:
    with tempfile.TemporaryDirectory() as out:
        await bsmr.build("//:dir", "--out", out)
        assert (Path(out) / "b.txt").exists()
        assert (Path(out) / "nested_dir" / "a.txt").exists()


@bsmr_test()
async def test_out_stdout_multiple(bsmr: Bsmr) -> None:
    result = await bsmr.build("//:a", "//:b", "--out", "-")

    # The e2e test runner adds a `--build-report` flag in order to be able
    # to parse out failures. In normal usage of `--out -` there wouldn't be this
    # extra line of JSON on the stdout, we'd _just_ get the requested outputs.
    a, b, build_report, trailing = result.stdout.split("\n")
    assert (a, b) == ("a", "b") or (a, b) == ("b", "a")
    assert build_report.startswith("{")
    assert trailing == ""


@bsmr_test()
async def test_out_stdout_none(bsmr: Bsmr) -> None:
    await bsmr.build("--out", "-")


@bsmr_test()
async def test_out_stdout_directory(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.build("//:dir", "--out", "-"),
        stderr_regex="produces a default output that is a directory, and cannot be sent to stdout",
    )
