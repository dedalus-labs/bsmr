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
import re
from pathlib import Path

import pytest
from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test
from bsmr.tests.e2e_util.helper.golden import golden


def _replace_timestamp(s: str) -> str:
    return re.sub(r"\b[0-9]+\b", "<NUMBER>", s)


@bsmr_test(skip_for_os=["windows"])
async def test_user_event_log_custom_output(bsmr: Bsmr, tmp_path: Path) -> None:
    local_log = tmp_path / "test.json"

    await bsmr.bxl(
        "root//:test.bxl:instant_event",
        "--user-event-log",
        str(local_log),
    )

    assert Path(local_log).exists()

    # do some basic validation - golden tests take care of better validation
    with open(local_log, "r") as f:
        results = f.read().splitlines()
        # assert these events can be loaded
        json.loads(results[0])["command_line_args"]
        json.loads(results[1])["StarlarkUserEvent"]
        json.loads(results[2])["StarlarkUserEvent"]


@bsmr_test(skip_for_os=["windows"])
async def test_user_event_log_with_actions(bsmr: Bsmr, tmp_path: Path) -> None:
    local_log = tmp_path / "test.json-lines"

    await bsmr.bxl(
        "root//:test.bxl:action",
        "--event-log",
        str(local_log),
    )

    results = (
        (await bsmr.log("show-user", str(Path(local_log).absolute())))
        .stdout.strip()
        .splitlines()[1:]
    )

    # Remove any durations
    a = json.loads(results[0])
    a["ActionExecutionEvent"]["duration_millis"] = "<NUMBER>"
    a["ActionExecutionEvent"]["input_materialization_duration_millis"] = "<NUMBER>"
    b = json.loads(results[1])
    b["BxlEnsureArtifactsEvent"]["duration_millis"] = "<NUMBER>"

    results = _replace_timestamp(f"{json.dumps(a)}\n{json.dumps(b)}")

    # Just validate the user events, let's skip the invocation record
    golden(
        output=results,
        rel_path="action_event.golden.json",
    )


@bsmr_test(skip_for_os=["windows"])
async def test_user_event_with_log_show_user(bsmr: Bsmr) -> None:
    await bsmr.bxl(
        "root//:test.bxl:instant_event",
    )

    results = (await bsmr.log("show-user")).stdout.strip().splitlines()[1:]

    results = _replace_timestamp("\n".join(results))

    # Just validate the user events, let's skip the invocation record
    golden(
        output=results,
        rel_path="instant_event.golden.json",
    )


@bsmr_test(skip_for_os=["windows"])
@pytest.mark.parametrize(
    "file_extension", [".json-lines", ".json-lines.gz", ".json-lines.zst"]
)
async def test_user_event_log_with_log_show_user_compatibility(
    bsmr: Bsmr,
    tmp_path: Path,
    file_extension: str,
) -> None:
    local_log = tmp_path / f"test.{file_extension}"

    await bsmr.bxl(
        "root//:test.bxl:instant_event",
        "--event-log",
        str(local_log),
    )

    results = (
        (await bsmr.log("show-user", str(Path(local_log).absolute())))
        .stdout.strip()
        .splitlines()[1:]
    )

    results = _replace_timestamp("\n".join(results))

    # Just validate the user events, let's skip the invocation record
    golden(
        output=results,
        rel_path="instant_event.golden.json",
    )


# Placeholder for tests to be listed successfully on Windows.
@bsmr_test()
async def test_noop(bsmr: Bsmr) -> None:
    return
