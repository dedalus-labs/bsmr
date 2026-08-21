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
from pathlib import Path

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


@bsmr_test()
async def test_bxl_actions(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//artifact_test/artifacts.bxl:artifact_test",
    )

    # FIXME(JakobDegen): The first assert doesn't test anything the second doesn't cover
    assert "<source artifact artifact_test/TARGETS.fixture>" in result.stdout
    assert "[<source artifact artifact_test/TARGETS.fixture>]" in result.stdout


@bsmr_test()
async def test_bxl_create_build_actions(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//actions_test:actions.bxl:build_actions_test",
        "--",
        "--content",
        "my_content",
    )
    assert (bsmr.cwd / Path(result.stdout.strip())).read_text() == "my_content"


@bsmr_test()
async def test_bxl_create_build_actions_with_content_based_path(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//actions_test:actions.bxl:build_actions_test",
        "--",
        "--content",
        "my_content",
        "--has_content_based_path",
        "true",
    )

    assert (bsmr.cwd / Path(result.stdout.strip())).read_text() == "my_content"


@bsmr_test()
async def test_resolve(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//resolve_test:resolve.bxl:resolve_test",
    )

    assert "a-string\n" == result.stdout


@bsmr_test(skip_for_os=["windows"])
async def test_bxl_declared_artifact_path(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//actions_test/declared_artifact_path.bxl:declared_artifact_path_test",
    )

    output = result.stdout.splitlines()
    # first line is result of get_path_without_materialization, second line is output of ctx.output.ensure
    assert output[0] == output[1]


@bsmr_test()
async def test_bxl_build_and_write(bsmr: Bsmr) -> None:
    # Performs a failed build and a successful action.
    res = await bsmr.bxl(
        "//actions_test:actions.bxl:build_and_write",
        "--",
        "--target",
        "actions_test:fail",
    )

    assert res.process.returncode == 0
    assert "BXL SUCCEEDED" in res.stderr


@bsmr_test()
async def test_bxl_not_show_bxl_succeeded(bsmr: Bsmr) -> None:
    res = await bsmr.run_bsmr_command(
        "-v=status",
        "bxl",
        "//actions_test:actions.bxl:build_and_write",
        "--",
        "--target",
        "actions_test:fail",
    )

    assert res.process.returncode == 0
    assert "BXL SUCCEEDED" not in res.stderr


@bsmr_test()
async def test_write_json_cell_path(bsmr: Bsmr) -> None:
    res = await bsmr.bxl("//actions_test:actions.bxl:write_json_cell_path")
    output_path = res.stdout.strip()
    with open(output_path, "r") as f:
        content = f.read()
    data = json.loads(content)
    expcted = {"root//resolve_test:buildable": ["root//resolve_test/foo.txt"]}
    assert data == expcted


@bsmr_test()
async def test_ensure_unbound_artifact(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.bxl(
            "//actions_test:ensure_unbound_artifact.bxl:ensure_unbound_artifact_test"
        ),
        stderr_regex="Artifact must be bound by now",
    )
