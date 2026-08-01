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


import json

from bsmr.tests.e2e_util.api.buck import Buck
from bsmr.tests.e2e_util.buck_workspace import buck_test


@buck_test(inplace=False)
async def test_bsmr_config_logging_disabled(buck: Buck) -> None:
    result = await buck.targets("//:")
    assert "starlark_log_bsmrconfig" not in result.stderr
    assert "starlark_log_all_bsmrconfigs" not in result.stderr


@buck_test(inplace=False)
async def test_bsmr_config_logging_enabled(buck: Buck) -> None:
    result = await buck.targets("//:", "--config", "bsmrconfig.log=test.read1")
    lines = [
        line
        for line in result.stderr.splitlines()
        if "starlark_log_bsmrconfig" in line
        # Unfortunately, "starlark_log_bsmrconfig" also shows up inside the stacktrace, so
        # try to exclude these lines here
        and "print(" not in line
    ]
    assert len(lines) == 1

    result = await buck.targets(
        "//:", "--config", "bsmrconfig.log=test.not_a_valid_bsmrconfig"
    )
    lines = [
        line for line in result.stderr.splitlines() if "starlark_log_bsmrconfig" in line
    ]
    assert len(lines) == 0


@buck_test(inplace=False)
async def test_bsmr_config_logging_enabled_json(buck: Buck) -> None:
    result = await buck.targets("//:", "--config", "bsmrconfig.log_json=test.read1")
    lines = [
        line for line in result.stderr.splitlines() if "starlark_log_bsmrconfig" in line
    ]
    assert len(lines) == 1, result.stderr
    # Terrible way to strip out the timestamp from the log line...
    read_config = json.loads(lines[0].split(maxsplit=1)[1].strip())[
        "starlark_log_bsmrconfig"
    ]
    assert read_config["cell"] == "root"

    result = await buck.targets(
        "//:", "--config", "bsmrconfig.log_json=test.not_a_valid_bsmrconfig"
    )
    lines = [
        line for line in result.stderr.splitlines() if "starlark_log_bsmrconfig" in line
    ]
    assert len(lines) == 0


@buck_test(inplace=False)
async def test_bsmr_config_logging_all_enabled(buck: Buck) -> None:
    result = await buck.targets(
        "//:",
        "--config",
        "bsmrconfig.log_all_in_json=true",
    )
    print(result.stderr)
    jsons = [
        # Terrible way to strip out the timestamp from the log line...
        json.loads(line.split(maxsplit=1)[1])["starlark_log_all_bsmrconfigs"]
        for line in result.stderr.splitlines()
        if '{"starlark_log_all_bsmrconfigs' in line
    ]
    filtered = [j for j in jsons if j["section"] == "test"]
    assert len(filtered) == 2
    for j in filtered:
        assert j["key"] in ["read1", "read2"]
        assert j["cell"] == "root"
