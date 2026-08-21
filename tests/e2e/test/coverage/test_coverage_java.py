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
from typing import Optional

import pytest
from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test

from .test_coverage_utils import collect_coverage_for

JAVA_TEST_TARGET = "upstream//xplat/test_frameworks/coverage/java/playground:SimpleTest"

EXTRA_BSMR_ARGS = [
    "--config",
    "junit_selective_coverage_rollout.is_enabled=true",
    # currently selective coverage is enabled via this flag
]


@pytest.mark.parametrize("mode", [None, "@upstream//mode/dev", "@upstream//mode/opt"])
@bsmr_test(inplace=True)
async def test_java_coverage_file_filter(
    bsmr: Bsmr, tmp_path: Path, mode: Optional[str]
) -> None:
    file_to_collect_coverage = "xplat/test_frameworks/coverage/java/playground/java_test/src/test/com/facebook/playground/SimpleTest.java"
    result = await collect_coverage_for(
        bsmr,
        tmp_path,
        target=JAVA_TEST_TARGET,
        mode=mode,
        folder_filter=[],
        file_filter=[file_to_collect_coverage],
        extra_bsmr_args=EXTRA_BSMR_ARGS,
    )

    assert set(result) == {file_to_collect_coverage}, (
        f"Only {file_to_collect_coverage} should have coverage, instead got coverage for {str(result)}"
    )


@pytest.mark.parametrize("mode", [None, "@upstream//mode/dev", "@upstream//mode/opt"])
@bsmr_test(inplace=True)
async def test_java_coverage_folder_filter(
    bsmr: Bsmr, tmp_path: Path, mode: Optional[str]
) -> None:
    folder_to_collect_coverage = "xplat/test_frameworks/coverage/java/playground/java_test/src/test/com/facebook/playground/nested"
    result = await collect_coverage_for(
        bsmr,
        tmp_path,
        target=JAVA_TEST_TARGET,
        mode=mode,
        folder_filter=[folder_to_collect_coverage],
        file_filter=[],
        extra_bsmr_args=EXTRA_BSMR_ARGS,
    )

    expected_files = {
        "xplat/test_frameworks/coverage/java/playground/java_test/src/test/com/facebook/playground/nested/Adder.java"
    }
    assert set(result) == expected_files, (
        f"Only {expected_files} should have coverage, instead got coverage for {str(result)}"
    )


@bsmr_test(inplace=True)
async def test_junit_test_selective_coverage_doesnt_produce_coverage(
    bsmr: Bsmr, tmp_path: Path
) -> None:
    paths = await collect_coverage_for(
        bsmr,
        tmp_path,
        "upstream//testing_frameworks/code_coverage/junit/com/facebook/testing_frameworks:test",
        file_filter=[
            "testing_frameworks/code_coverage/junit/com/facebook/testing_frameworks/AddTest.java"
        ],
        folder_filter=[],
    )

    assert len(paths) == 0, str(paths)
