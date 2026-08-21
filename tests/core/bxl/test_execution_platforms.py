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
import random
import string
from pathlib import Path

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


@bsmr_test(write_invocation_record=True)
async def test_bxl_exec_platform_dynamic_output(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//executor_fallback_tests/dynamic.bxl:test_dynamic_output",
        "-c",
        f"test.cache_buster={random_string()}",
        "--local-only",
    )

    output = result.stdout.splitlines()[0]
    assert os.path.exists(bsmr.cwd / Path(output))

    res = await expect_failure(
        bsmr.bxl(
            "//executor_fallback_tests/dynamic.bxl:test_dynamic_output",
            "-c",
            f"test.cache_buster={random_string()}",
            "--remote-only",
        ),
        stderr_regex="Incompatible executor preferences",
    )

    record = res.invocation_record()
    errors = record["errors"]

    assert len(errors) == 1
    assert errors[0]["category"] == "USER"


@bsmr_test()
async def test_bxl_execution_platforms(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "//executor_fallback_tests/test.bxl:test_exec_platforms",
        "-c",
        f"test.cache_buster={random_string()}",
        "--",
        "--exec_deps",
        "//executor_fallback_tests:remote_only",
    )

    output = result.stdout.splitlines()[0]
    assert os.path.exists(bsmr.cwd / Path(output))

    await expect_failure(
        bsmr.bxl(
            "//executor_fallback_tests/test.bxl:test_exec_platforms",
            "-c",
            f"test.cache_buster={random_string()}",
            "--",
            "--exec_deps",
            "//executor_fallback_tests:local_only",
        )
    )

    result = await bsmr.bxl(
        "//executor_fallback_tests/test.bxl:test_exec_platforms",
        "-c",
        f"test.cache_buster={random_string()}",
        "--",
        "--toolchains",
        "//executor_fallback_tests:remote_only_toolchain",
    )

    output = result.stdout.splitlines()[0]
    assert os.path.exists(bsmr.cwd / Path(output))

    await expect_failure(
        bsmr.bxl(
            "//executor_fallback_tests/test.bxl:test_exec_platforms",
            "-c",
            f"test.cache_buster={random_string()}",
            "--",
            "--toolchains",
            "//executor_fallback_tests:local_only_toolchain",
        )
    )

    result = await bsmr.bxl(
        "//executor_fallback_tests/test.bxl:test_exec_compatible_with",
        "-c",
        f"test.cache_buster={random_string()}",
    )

    output = result.stdout.splitlines()[0]
    assert os.path.exists(bsmr.cwd / Path(output))

    await expect_failure(
        bsmr.bxl(
            "//executor_fallback_tests/test.bxl:test_exec_compatible_with",
            "-c",
            f"test.cache_buster={random_string()}",
            "--remote-only",
        )
    )


def random_string() -> str:
    return "".join(random.choice(string.ascii_lowercase) for i in range(256))
