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

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


@bsmr_test(inplace=True)
async def test_python_coverage(bsmr: Bsmr) -> None:
    with tempfile.NamedTemporaryFile("w") as covfile:
        await bsmr.test(
            "@upstream//mode/dbgo-cov",
            "root//tests/targets/rules/python/coverage:test",
            "--",
            "--collect-coverage",
            f"--coverage-output={covfile.name}",
        )
        paths = []
        with open(covfile.name) as results:
            for line in results:
                paths.append(json.loads(line)["filepath"])
    assert "fbcode/bsmr/tests/targets/rules/python/coverage/lib.py" in paths, str(
        paths
    )


@bsmr_test(inplace=True)
async def test_python_coverage_filtering_by_folder(bsmr: Bsmr) -> None:
    folder_to_collect = "bsmr/tests/targets/rules/python/coverage"
    with tempfile.NamedTemporaryFile("w") as covfile:
        await bsmr.test(
            "@upstream//mode/dbgo-cov",
            "root//tests/targets/rules/python/coverage:test",
            "-c",
            f"fbcode.cxx_coverage_only={folder_to_collect}",
            "--",
            "--collect-coverage",
            f"--coverage-output={covfile.name}",
        )
        paths = []
        with open(covfile.name) as results:
            for line in results:
                paths.append(json.loads(line)["filepath"])
    assert set(paths) == {
        f"fbcode/{folder_to_collect}/lib.py",
        f"fbcode/{folder_to_collect}/test.py",
    }, (
        f"Only folder fbcode/{folder_to_collect} should have coverage, instead got coverage for {str(paths)}"
    )
