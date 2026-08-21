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


import subprocess
from pathlib import Path
from typing import List

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


@bsmr_test()
async def test_run_executable(bsmr: Bsmr) -> None:
    result = await bsmr.run("root//:print_hello")
    assert result.stdout.strip() == "hello"


@bsmr_test(skip_for_os=["windows"])
async def test_emit_shell(bsmr: Bsmr) -> None:
    result = await bsmr.run(
        "root//:print_hello",
        "--emit-shell",
    )

    out = subprocess.check_output(result.stdout, shell=True, encoding="utf-8")
    assert out.strip() == "hello"


@bsmr_test(write_invocation_record=True)
async def test_run_non_executable_fails(bsmr: Bsmr) -> None:
    res = await expect_failure(
        bsmr.run(
            "root//:no_run_info",
        ),
        stderr_regex=r"Target `[^`]+` is not a binary rule \(only binary rules can be `run`\)",
    )

    record = res.invocation_record()
    [error] = record["errors"]

    assert error["category_key"] == "RunCommandError::NonBinaryRule"
    assert error["category"] == "USER"


@bsmr_test(write_invocation_record=True)
async def test_run_exit_result(bsmr: Bsmr) -> None:
    res = await bsmr.run(
        "root//:print_hello",
    )
    record = res.invocation_record()
    assert record["exit_result_name"] == "EXEC"


@bsmr_test(allow_soft_errors=True)
async def test_passing_arguments(bsmr: Bsmr) -> None:
    async def f(args1: List[str], args2: List[str]) -> None:
        result = await bsmr.run("root//:echo_args", *args1, *args2)
        assert result.stdout.strip() == " ".join(args2)

    await f(["--"], ["val", "--long", "-s", "spa  ces"])
    await f(["--"], ["val", "--", "test"])
    # Without --, a deprecation warning is emitted but command still succeeds
    result = await bsmr.run("root//:echo_args", "val", "--long")
    assert result.stdout.strip() == "val --long"
    assert "will require" in result.stderr
    await f([], ["val", "--", "x"])  # Would work differently in Legacy (no -- to user)
    await expect_failure(
        bsmr.run("root//:echo_args", "--not-a-flag"),
        stderr_regex=r"unexpected argument '--not-a-flag'",
    )


@bsmr_test()
async def test_executable_fail_to_build(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.run("root//:build_fail"),
        stderr_regex=r"Failed to build",
    )


@bsmr_test(allow_soft_errors=True)
async def test_run_args_without_separator_warning(bsmr: Bsmr) -> None:
    result = await bsmr.run("root//:echo_args", "my_arg")
    assert result.stdout.strip() == "my_arg"
    assert "will require" in result.stderr


@bsmr_test()
async def test_input(bsmr: Bsmr) -> None:
    await bsmr.run("root//:check_input_test", input=b"test")


@bsmr_test()
async def test_change_cwd(bsmr: Bsmr, tmp_path: Path) -> None:
    result = await bsmr.run(
        "root//:print_cwd",
        f"--chdir={tmp_path}",
    )
    assert tmp_path.resolve() == Path(result.stdout.strip()).resolve()


@bsmr_test()
async def test_dont_change_cwd(bsmr: Bsmr) -> None:
    result = await bsmr.run("root//:print_cwd")
    assert bsmr.cwd == Path(result.stdout.strip())
