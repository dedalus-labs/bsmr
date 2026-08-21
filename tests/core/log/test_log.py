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


import json
import os.path
from pathlib import Path

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test
from bsmr.tests.e2e_util.helper.utils import is_running_on_windows


@bsmr_test()
async def test_log_show_invocation_record(bsmr: Bsmr, tmp_path: Path) -> None:
    mode_file = tmp_path / "mode"
    mode_file.write_text("-c\naa.bb=cc\n-c\ndd.ee=ff\n")

    # Any simple would do.
    await bsmr.uquery(f"@{mode_file}", "//:EEE")

    result = await bsmr.log("show")
    invocation = json.loads(result.stdout.splitlines()[0])
    command_line_args = invocation["command_line_args"]
    expanded_command_line_args = invocation["expanded_command_line_args"]
    assert f"@{mode_file}" in command_line_args
    assert f"@{mode_file}" not in expanded_command_line_args
    assert "aa.bb=cc" in expanded_command_line_args
    assert "aa.bb=cc" not in command_line_args


@bsmr_test(write_invocation_record=True)
async def test_log_size_logging(bsmr: Bsmr) -> None:
    res = await bsmr.cquery(
        "//:EEE",
    )

    out = await bsmr.log("last")
    path = out.stdout.strip()
    with open(path, "rb") as f:
        log_size_in_disk = len(f.read())

    logged_size = res.invocation_record()["compressed_event_log_size_bytes"]

    assert logged_size == log_size_in_disk


@bsmr_test()
async def test_last_log(bsmr: Bsmr) -> None:
    await bsmr.build("//:EEE")
    out = await bsmr.log("last")
    path = out.stdout.strip()
    assert os.path.exists(path)
    assert "/log/" in path or "\\log\\" in path
    out2 = await bsmr.log("path")
    assert path == out2.stdout.strip()


@bsmr_test()
async def test_last_log_all(bsmr: Bsmr) -> None:
    await bsmr.build("//:EEE")
    out = await bsmr.log("last", "--all")
    paths = list(out.stdout.splitlines())
    assert len(paths) > 0
    for path in paths:
        assert os.path.exists(path)
        assert "/log/" in path or "\\log\\" in path


@bsmr_test()
async def test_log_command_with_trace_id(bsmr: Bsmr, tmp_path: Path) -> None:
    build_file_path = tmp_path / "b"
    await bsmr.uquery("//:", f"--write-build-id={build_file_path}")
    build_id = build_file_path.read_text("utf-8").strip()
    await bsmr.log("show", f"--trace-id={build_id}")
    log = (await bsmr.log("show", f"--trace-id={build_id}")).stdout.strip().splitlines()
    # Check it looks like log.
    assert len(log) >= 1
    for line in log:
        json.loads(line)


@bsmr_test()
async def test_what_bsmr(bsmr: Bsmr, tmp_path: Path) -> None:
    mode_path = tmp_path / "mode"
    mode_path.write_text("-c\nxx.yy=zz\n")

    await bsmr.uquery("//:", f"@{mode_path}")

    out = await bsmr.log("what-cmd")
    assert "uquery //: " in out.stdout
    if not is_running_on_windows():
        # Path is quoted on Windows.
        assert f"uquery //: @{mode_path}" in out.stdout

    out = await bsmr.log("what-cmd", "--expand")
    assert "uquery //: -c" in out.stdout
