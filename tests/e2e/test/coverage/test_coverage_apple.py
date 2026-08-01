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

from bsmr.tests.e2e_util.api.buck import Buck
from bsmr.tests.e2e_util.buck_workspace import buck_test


@buck_test(inplace=True)
async def test_apple_coverage(buck: Buck) -> None:
    with tempfile.NamedTemporaryFile("w") as covfile:
        await buck.test(
            "-c",
            "xplat.available_platforms=APPLE,CXX",
            "-c",
            "code_coverage.enable=all",
            "upstream//fbobjc/Samples/TestInfra/TpxUnitTests:TpxUnitTests",
            "--",
            "--collect-coverage",
            f"--coverage-output={covfile.name}",
        )
        paths = []
        with open(covfile.name) as results:
            for line in results:
                paths.append(json.loads(line)["filepath"])
    assert (
        "fbobjc/Samples/TestInfra/TpxUnitTests/TpxUnitTests/TpxUnitTests.m" in paths
    ), str(paths)


@buck_test(inplace=True)
async def test_apple_coverage_xplat(buck: Buck) -> None:
    with tempfile.NamedTemporaryFile("w") as covfile:
        await buck.test(
            "-c",
            "xplat.available_platforms=APPLE,CXX",
            "-c",
            "code_coverage.enable=all",
            # By default, xplat targets currently use xbat to compile to apple.
            # With xbat, however, we'll get divergence between the LLVM tooling
            # during pika upgrades, as pika will be newer than what's provided
            # by xbat.
            "@upstream//fbobjc/mode/bsmr/toolchains/pika-fat",
            "upstream//xplat/testinfra/playground/cpp:example_testApple",
            "--",
            "--collect-coverage",
            f"--coverage-output={covfile.name}",
        )
        paths = []
        with open(covfile.name) as results:
            for line in results:
                paths.append(json.loads(line)["filepath"])
    assert "xplat/testinfra/playground/cpp/ExampleTest.cpp" in paths, str(paths)
