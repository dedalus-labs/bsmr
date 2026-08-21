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
import os
import re
from pathlib import Path

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test
from bsmr.tests.e2e_util.helper.golden import (
    golden,
    sanitize_build_report,
    sanitize_hashes,
)


def _replace_hash(s: str) -> str:
    return re.sub(r"\b[0-9a-f]{16}\b", "<HASH>", s)


OUTPUT_ROOT_REL_PATH = "bsmr-out/default/art/root"


@bsmr_test()
async def test_bxl_ensure_no_materialization(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//no_materialization_bxl_build/remote_text.bxl:ensure",
        "--materializations=none",
    )

    [output] = result.stdout.splitlines()
    assert os.path.exists(bsmr.cwd / Path(output)) is False

    result = await bsmr.bxl(
        "//no_materialization_bxl_build/remote_text.bxl:ensure",
    )

    [output] = result.stdout.splitlines()
    assert os.path.exists(bsmr.cwd / Path(output)) is True


@bsmr_test()
async def test_bxl_ensure(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//ensure.bxl:ensure_build_result_test",
        "--",
        "--target",
        ":buildable",
    )

    outputs = json.loads(result.stdout)
    [output] = [v for (k, v) in outputs.items() if k.startswith("root//:buildable")][
        0
    ]
    assert (bsmr.cwd / Path(output)).read_text() == "abcd"

    result = await bsmr.bxl(
        "//ensure.bxl:ensure_cmd_line_test",
    )

    lines = sorted(result.stdout.splitlines())
    assert (bsmr.cwd / Path(lines[0])).read_text() == "run_info_out"
    assert (bsmr.cwd / Path(lines[1])).read_text() == "target_with_tset\n"
    assert (bsmr.cwd / Path(lines[2])).read_text() == "tset1\n"
    assert (bsmr.cwd / Path(lines[3])).read_text() == "tset2\n"
    assert (bsmr.cwd / Path(lines[4])).read_text() == "tset3\n"

    result = await bsmr.bxl(
        "//ensure.bxl:ensure_cmd_line_json_output",
    )

    json_array = sorted(json.loads(result.stdout))
    assert "target_with_tset" in json_array[0]
    assert "tset1" in json_array[1]
    assert "tset2" in json_array[2]
    assert "tset3" in json_array[3]


@bsmr_test(skip_for_os=["windows"])
async def test_bxl_artifact_path(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//artifacts.bxl:artifact_path_test",
    )

    outputs = json.loads(result.stdout)

    assert outputs["source_artifact"] == "<source artifact artifacts/DATA>"
    # The project relative path of the source artifact
    assert outputs["source_artifact_project_rel_path"] == "artifacts/DATA"

    # Abs path for the source artifact. The path should exist on the filesystem.
    assert outputs["source_artifact_abs_path"] == str(bsmr.cwd / Path("artifacts/DATA"))
    assert (
        os.path.exists((bsmr.cwd / Path(outputs["source_artifact_abs_path"]))) is True
    )

    assert (
        "build artifact foo.txt bound to root//artifacts:with_build_artifact"
        in outputs["build_artifact"]
    )

    prefix = OUTPUT_ROOT_REL_PATH + "/"

    # The project relative path to the bsmr-out directory with the output
    assert outputs["build_artifact_project_rel_path"].startswith(prefix)
    assert (
        "/artifacts/__with_build_artifact__/foo.txt"
        in outputs["build_artifact_project_rel_path"]
    )

    # Abs path for the build artifact. Path should not exist on the filesystem since it's not materialized.
    assert outputs["build_artifact_abs_path"] == str(
        bsmr.cwd / Path(outputs["build_artifact_project_rel_path"])
    )

    assert (
        os.path.exists((bsmr.cwd / Path(outputs["build_artifact_abs_path"]))) is False
    )


@bsmr_test(skip_for_os=["windows"])
async def test_bxl_artifact_path_cmd_args(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//artifacts.bxl:cmd_args_artifact_path_test",
    )

    outputs = json.loads(result.stdout)
    _test_bxl_artifact_path_cmd_args_helper(
        bsmr,
        "kind/__target_with_outputs__/run_info_out",
        outputs["target_with_outputs_rel_paths"][0],
        False,
    )

    _test_bxl_artifact_path_cmd_args_helper(
        bsmr,
        "kind/__target_with_outputs__/run_info_out",
        outputs["target_with_outputs_abs_paths"][0],
        True,
    )

    assert len(outputs["target_with_tset_rel_paths"]) == 4

    _test_bxl_artifact_path_cmd_args_helper(
        bsmr,
        "kind/__target_with_tset__/out.txt",
        outputs["target_with_tset_rel_paths"][0],
        False,
    )
    _test_bxl_artifact_path_cmd_args_helper(
        bsmr,
        "kind/__tset1__/out.txt",
        outputs["target_with_tset_rel_paths"][1],
        False,
    )
    _test_bxl_artifact_path_cmd_args_helper(
        bsmr,
        "kind/__tset2__/out.txt",
        outputs["target_with_tset_rel_paths"][2],
        False,
    )
    _test_bxl_artifact_path_cmd_args_helper(
        bsmr,
        "kind/__tset3__/out.txt",
        outputs["target_with_tset_rel_paths"][3],
        False,
    )

    assert len(outputs["target_with_tset_abs_paths"]) == 4

    _test_bxl_artifact_path_cmd_args_helper(
        bsmr,
        "kind/__target_with_tset__/out.txt",
        outputs["target_with_tset_abs_paths"][0],
        True,
    )
    _test_bxl_artifact_path_cmd_args_helper(
        bsmr,
        "kind/__tset1__/out.txt",
        outputs["target_with_tset_abs_paths"][1],
        True,
    )
    _test_bxl_artifact_path_cmd_args_helper(
        bsmr,
        "kind/__tset2__/out.txt",
        outputs["target_with_tset_abs_paths"][2],
        True,
    )
    _test_bxl_artifact_path_cmd_args_helper(
        bsmr,
        "kind/__tset3__/out.txt",
        outputs["target_with_tset_abs_paths"][3],
        True,
    )


def _test_bxl_artifact_path_cmd_args_helper(
    bsmr: Bsmr, part_to_validate: str, full_path: str, is_abs: bool
) -> None:
    assert OUTPUT_ROOT_REL_PATH in full_path
    assert part_to_validate in full_path
    if is_abs:
        assert str((bsmr.cwd / Path(OUTPUT_ROOT_REL_PATH))) in full_path
        assert os.path.exists(full_path) is False
    else:
        assert str(bsmr.cwd) not in full_path
        assert os.path.exists((bsmr.cwd / Path(full_path))) is False


@bsmr_test(allow_soft_errors=True, skip_for_os=["darwin", "windows"])
async def test_bxl_ensure_failures(bsmr: Bsmr, tmp_path: Path) -> None:
    """Test that BXL fails when trying to ensure a failed build artifact."""
    report = tmp_path / "build-report.json"

    await expect_failure(
        bsmr.bxl(
            "//ensure.bxl:ensure_failures",
            "--build-report",
            str(report),
        ),
    )

    with open(report) as f:
        build_report = json.loads(f.read())

    sanitize_build_report(build_report)

    golden(
        output=sanitize_hashes(json.dumps(build_report, indent=2, sort_keys=True)),
        rel_path="fixtures/test_bxl_ensure_failures.golden.json",
    )
