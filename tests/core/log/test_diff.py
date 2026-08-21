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
import tempfile
import typing

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test
from bsmr.tests.e2e_util.helper.utils import filter_events


def with_bsmr_output(output: str) -> typing.List[str]:
    return [
        "-c",
        f"test.bsmr_output={output}",
    ]


def with_bsmr_key_value(key: str, value: str) -> typing.List[str]:
    return [
        "-c",
        f"test.{key}={value}",
    ]


def parse_json_diffs(stdout: str) -> typing.List[dict]:
    # first three lines are the header
    lines = stdout.splitlines()[3:]
    return [json.loads(line) for line in lines if line.strip()]


@bsmr_test()
async def test_no_action_divergence_command(bsmr: Bsmr) -> None:
    await bsmr.build("//:simple", *with_bsmr_output("foo"))
    out1 = await bsmr.log("last")
    path1 = out1.stdout.strip()
    await bsmr.build("//:simple", *with_bsmr_output("foo"))
    out2 = await bsmr.log("last")
    path2 = out2.stdout.strip()
    out = await bsmr.log(
        "diff", "action-divergence", "--path1", path1, "--path2", path2
    )

    assert "No divergent actions found." in out.stdout


@bsmr_test()
async def test_action_divergence_command(bsmr: Bsmr) -> None:
    await bsmr.build("//:non_det", *with_bsmr_output("foo"))
    await bsmr.build("//:non_det", *with_bsmr_output("bar"))
    out = await bsmr.log(
        "diff", "action-divergence", "--recent1", "0", "--recent2", "1"
    )

    assert (
        "Present in both builds with differing output digests\nprelude//:non_det (<unspecified>) (write foo.txt)"
        in out.stdout
    )


@bsmr_test()
async def test_no_config_diff_command(bsmr: Bsmr) -> None:
    await bsmr.build("//:simple", *with_bsmr_output("foo"))
    out1 = await bsmr.log("last")
    path1 = out1.stdout.strip()
    await bsmr.build("//:simple", *with_bsmr_output("foo"))
    out2 = await bsmr.log("last")
    path2 = out2.stdout.strip()
    out = await bsmr.log(
        "diff",
        "external-configs",
        "--path1",
        path1,
        "--path2",
        path2,
        "--format=json",
    )
    diffs = parse_json_diffs(out.stdout)
    assert len(diffs) == 0


@bsmr_test()
async def test_diff_order_config_diff_command(bsmr: Bsmr) -> None:
    await bsmr.build(
        "//:simple",
        *with_bsmr_output("out"),
        *with_bsmr_key_value("key_a", "1"),
        *with_bsmr_key_value("key_b", "2"),
    )
    out1 = await bsmr.log("last")
    path1 = out1.stdout.strip()
    await bsmr.build(
        "//:simple",
        *with_bsmr_output("out"),
        *with_bsmr_key_value("key_b", "2"),
        *with_bsmr_key_value("key_a", "1"),
    )
    out2 = await bsmr.log("last")
    path2 = out2.stdout.strip()
    out = await bsmr.log(
        "diff",
        "external-configs",
        "--path1",
        path1,
        "--path2",
        path2,
        "--format=json",
    )
    diffs = parse_json_diffs(out.stdout)
    assert len(diffs) == 1 and "FullDiff" in diffs[0]

    changes = diffs[0]["FullDiff"]["changes"]
    keys = {"test.key_a=1", "test.key_b=2"}
    original_order = [
        c["value"]
        for c in changes
        if c["tag"] in ("equal", "delete") and c["value"] in keys
    ]
    new_order = [
        c["value"]
        for c in changes
        if c["tag"] in ("equal", "insert") and c["value"] in keys
    ]
    assert original_order == ["test.key_a=1", "test.key_b=2"]
    assert new_order == ["test.key_b=2", "test.key_a=1"]


@bsmr_test()
async def test_config_diff_command_command_line(bsmr: Bsmr) -> None:
    await bsmr.build(
        "//:simple",
        *with_bsmr_output("changed_old"),
        *with_bsmr_key_value("first", "x"),
        *with_bsmr_key_value("first", "overwrite_x"),
    )
    out1 = await bsmr.log("last")
    path1 = out1.stdout.strip()
    with tempfile.NamedTemporaryFile("w", delete=False) as f:
        f.write("[test]\n")
        f.write("second = x\n")
        f.close()
    await bsmr.build(
        "//:simple",
        *with_bsmr_output("changed_new"),
        "--config-file",
        f.name,
    )
    out2 = await bsmr.log("last")
    path2 = out2.stdout.strip()
    out = await bsmr.log(
        "diff",
        "external-configs",
        "--path1",
        path1,
        "--path2",
        path2,
        "--format=json",
    )
    summary_diffs = [d for d in parse_json_diffs(out.stdout) if "FullDiff" not in d]
    assert len(summary_diffs) == 3

    assert (
        summary_diffs[0]["Changed"]["key"] == "test.bsmr_output"
        and summary_diffs[0]["Changed"]["old_value"] == "changed_old"
        and summary_diffs[0]["Changed"]["new_value"] == "changed_new"
    )

    assert (
        summary_diffs[1]["FirstOnly"]["key"] == "test.first"
        and summary_diffs[1]["FirstOnly"]["value"] == "overwrite_x"
    )
    assert (
        summary_diffs[2]["SecondOnly"]["key"] == "test.second"
        and summary_diffs[2]["SecondOnly"]["value"] == "x"
    )


@bsmr_test()
async def test_config_diff_command_project_relative(bsmr: Bsmr) -> None:
    await bsmr.build(
        "//:simple",
        *with_bsmr_output("out"),
        "@root//mode/my_mode_a",
        "@root//mode/my_mode_c",
    )
    out1 = await bsmr.log("last")
    path1 = out1.stdout.strip()
    await bsmr.build(
        "//:simple",
        *with_bsmr_output("out"),
        "@root//mode/my_mode_b",
        "@root//mode/my_mode_c",
        "@root//mode/my_mode_b",
    )
    out2 = await bsmr.log("last")
    path2 = out2.stdout.strip()
    out = await bsmr.log(
        "diff",
        "external-configs",
        "--path1",
        path1,
        "--path2",
        path2,
        "--format=json",
    )
    summary_diffs = [d for d in parse_json_diffs(out.stdout) if "FullDiff" not in d]
    assert len(summary_diffs) == 2

    first_only = {d["FirstOnly"]["key"] for d in summary_diffs if "FirstOnly" in d}
    second_only = {d["SecondOnly"]["key"] for d in summary_diffs if "SecondOnly" in d}
    # We only store the path of the modefile
    assert first_only == {"my_mode_a.bcfg"}
    assert second_only == {"my_mode_b.bcfg"}


@bsmr_test(write_invocation_record=True)
async def test_config_diff_tracker_modfile_change(bsmr: Bsmr) -> None:
    await bsmr.build(
        "//:simple",
        *with_bsmr_output("out"),
        "@root//mode/my_mode_a",
    )
    res = await bsmr.build(
        "//:simple",
        *with_bsmr_output("out"),
        "@root//mode/my_mode_b",
    )
    cell_config_diffs = await filter_events(
        bsmr, "Event", "data", "Instant", "data", "CellHasNewConfigs"
    )
    assert len(cell_config_diffs) == 1 and cell_config_diffs[0]["cell"] == "prelude"

    assert res.invocation_record()["new_configs_used"] == 1


@bsmr_test(write_invocation_record=True)
async def test_config_diff_tracker_no_change(bsmr: Bsmr) -> None:
    await bsmr.build(
        "//:simple",
        *with_bsmr_output("out"),
        "@root//mode/my_mode_a",
    )
    res = await bsmr.build(
        "//:simple",
        *with_bsmr_output("out"),
        "@root//mode/my_mode_a",
    )
    cell_config_diffs = await filter_events(
        bsmr, "Event", "data", "Instant", "data", "CellHasNewConfigs"
    )
    assert len(cell_config_diffs) == 0
    assert res.invocation_record()["new_configs_used"] == 0
