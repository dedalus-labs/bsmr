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


import asyncio
import json
import os
import re
import signal
from pathlib import Path

import pytest
from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.api.bsmr_result import BsmrException, ExitCodeV2
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import (
    bsmr_test,
    env,
    get_mode_from_platform,
    is_deployed_bsmr,
)

MAC_AND_WINDOWS = ["darwin", "windows"]


def remove_ansi_escape_sequences(ansi_str: str) -> str:
    """convert ansi_str to str"""
    ansi_escape = re.compile(r"\x1B(?:[@-Z\\-_]|\[[0-?]*[ -/]*[@-~])")
    return ansi_escape.sub("", ansi_str)


# TODO(marwhal): Fix and enable on Windows
@bsmr_test(inplace=True, skip_for_os=["windows"])
async def test_sh_test(bsmr: Bsmr) -> None:
    await bsmr.test(
        "root//tests/targets/rules/sh_test:test",
    )

    await expect_failure(
        bsmr.test(
            "root//tests/targets/rules/sh_test:test_fail",
        ),
        stderr_regex=r"1 TESTS FAILED\n(\s)+✗ fbcode\/\/bsmr\/tests\/targets\/rules\/sh_test:test_fail - unmanaged",
    )


# TODO(marwhal): Fix and enable on Windows
@bsmr_test(inplace=True, skip_for_os=["windows"])
async def test_sh_test_remote_checks(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.test(
            "root//tests/targets/rules/sh_test:test",
            "--remote-only",
        ),
        stderr_regex="Incompatible executor preferences: `RemoteRequired` & `LocalRequired`",
    )
    await bsmr.test(
        "root//tests/targets/rules/sh_test:test_remote_implicit",
        "--local-only",
    )
    await bsmr.test(
        "root//tests/targets/rules/sh_test:test_remote_implicit",
        "--remote-only",
    )
    await expect_failure(
        bsmr.test(
            "root//tests/targets/rules/sh_test:test_remote_explicit",
            "--local-only",
        ),
        stderr_regex="LocalOnly.*is incompatible",
    )
    await bsmr.test(
        "root//tests/targets/rules/sh_test:test_remote_explicit",
        "--remote-only",
    )


# TODO(marwhal): Fix and enable on Windows
@bsmr_test(inplace=True, skip_for_os=["windows"])
async def test_test_build_fail(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.test(
            "hewwo_bsmr",
        ),
        stderr_regex="does not exist",
    )

    await bsmr.test("root//tests/targets/rules/sh_test:test")


@bsmr_test(inplace=True, skip_for_os=["darwin"])
async def test_cpp_test(bsmr: Bsmr) -> None:
    mode = get_mode_from_platform()
    await bsmr.test("root//tests/targets/rules/cxx:cpp_test_pass", mode)

    await expect_failure(
        bsmr.test("root//tests/targets/rules/cxx:cpp_test_fail", mode),
        stderr_regex=r"1 TESTS FAILED\n(\s)+✗ fbcode\/\/bsmr\/tests\/targets\/rules\/cxx:cpp_test_fail - Simple\.Fail",
    )

    await bsmr.test("root//tests/targets/rules/cxx:cpp_test_local_only", mode)

    await expect_failure(
        bsmr.test(
            "root//tests/targets/rules/cxx:cpp_test_local_only",
            mode,
            "--remote-only",
        ),
        stderr_regex=r"The desired execution strategy \(.RemoteOnly.\) is incompatible with the executor config that was selected",
    )


@bsmr_test(inplace=True, skip_for_os=["windows"])
async def test_cpp_stress_runs(bsmr: Bsmr) -> None:
    mode = get_mode_from_platform()
    res = await bsmr.test(
        "root//tests/targets/rules/cxx:cpp_test_pass",
        mode,
        "--",
        "--stress-runs=10",
    )

    assert "Pass 10" in res.stderr, "Expected stress runs to be run"


@bsmr_test(inplace=True, skip_for_os=["windows"])
async def test_cpp_stress_runs_deterministic_paths(bsmr: Bsmr) -> None:
    mode = get_mode_from_platform()
    res = await bsmr.test(
        "root//tests/targets/rules/cxx:cpp_test_pass",
        mode,
        "--",
        "--stress-runs=10",
    )

    assert "Pass 10" in res.stderr, "Expected stress runs to be run"


@bsmr_test(inplace=True, skip_for_os=["darwin"])
async def test_cpp_test_fdb_message(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.test(
            "root//tests/targets/rules/cxx:cpp_test_fail",
            get_mode_from_platform(),
            "--",
            "--color",
            "off",
        ),
        stderr_regex=r"Run \$ fdb bsmr test \<args\> to debug",
    )


@bsmr_test(inplace=True, skip_for_os=MAC_AND_WINDOWS)
async def test_python_test(bsmr: Bsmr) -> None:
    await bsmr.test("root//tests/targets/rules/python/test:test")

    await bsmr.test("root//tests/targets/rules/python/test:test_env")

    await expect_failure(
        bsmr.test("root//tests/targets/rules/python/test:test_fail"),
        stderr_regex=r"1 TESTS FAILED\n(\s)+✗ fbcode\/\/bsmr\/tests\/targets\/rules\/python\/test:test_fail - test",
    )

    await expect_failure(
        bsmr.test("root//tests/targets/rules/python/test:test_fatal"),
        stderr_regex=r"1 TESTS FATALS\n(\s)+⚠ fbcode\/\/bsmr\/tests\/targets\/rules\/python\/test:test_fatal - test",
    )


@bsmr_test(inplace=True, skip_for_os=MAC_AND_WINDOWS)
async def test_python_test_with_remote_execution(bsmr: Bsmr) -> None:
    await bsmr.test(
        "root//tests/targets/rules/python/test:test_remote_execution",
    )
    await expect_failure(
        bsmr.test(
            "root//tests/targets/rules/python/test:test_remote_execution_fail",
        ),
        stderr_regex=r"1 TESTS FAILED\n(\s)+✗ fbcode\/\/bsmr\/tests\/targets\/rules\/python\/test:test_remote_execution_fail - test",
    )
    await expect_failure(
        bsmr.test(
            "root//tests/targets/rules/python/test:test_remote_execution_fatal",
        ),
        stderr_regex=r"1 TESTS FATALS\n(\s)+⚠ fbcode\/\/bsmr\/tests\/targets\/rules\/python\/test:test_remote_execution_fatal - test",
    )


@bsmr_test(inplace=True, skip_for_os=MAC_AND_WINDOWS)
async def test_python_needed_coverage(bsmr: Bsmr) -> None:
    await bsmr.test(
        "root//tests/targets/rules/python/needed_coverage:test_pass",
        "root//tests/targets/rules/python/needed_coverage:test_pass_specific_file",
    )
    await expect_failure(
        bsmr.test("root//tests/targets/rules/python/needed_coverage:test_fail"),
        stderr_regex="ERROR: Actual coverage [0-9.]*% is smaller than expected 100.% for file",
    )
    await expect_failure(
        bsmr.test(
            "root//tests/targets/rules/python/needed_coverage:test_fail_fractional"
        ),
        stderr_regex="ERROR: Actual coverage [0-9.]*% is smaller than expected [0-9.]*% for file",
    )


@bsmr_test(inplace=True, skip_for_os=MAC_AND_WINDOWS)
async def test_tests_attribute(bsmr: Bsmr) -> None:
    lib_tests = await bsmr.test("root//tests/targets/rules/python/test:lib")
    assert "Pass 1" in remove_ansi_escape_sequences(lib_tests.stderr)


@bsmr_test(inplace=True, skip_for_os=MAC_AND_WINDOWS)
async def test_tests_attribute_ignore(bsmr: Bsmr) -> None:
    lib_tests = await bsmr.test(
        "root//tests/targets/rules/python/test:lib",
        "--ignore-tests-attribute",
    )
    assert "NO TESTS RAN" in remove_ansi_escape_sequences(lib_tests.stderr)


@bsmr_test(inplace=True)
async def test_listing_failure(bsmr: Bsmr) -> None:
    output = await expect_failure(
        bsmr.test(
            "root//tests/targets/rules/python/broken:broken",
            get_mode_from_platform(),
        ),
    )
    assert re.search(r"Listing Fail 1", output.stderr)
    assert re.search(
        r"1 LISTINGS FAILED\n(\s)+⚠ fbcode\/\/bsmr\/tests\/targets\/rules\/python\/broken:broken\n",
        output.stderr,
        re.DOTALL,
    )


@bsmr_test(inplace=True, skip_for_os=["windows"])
async def test_python_import_error_with_static_listing_builtin_runner(
    bsmr: Bsmr,
) -> None:
    output = await expect_failure(
        bsmr.test(
            "root//tests/targets/rules/python/broken:broken_with_static_listing_builtin_runner",
            get_mode_from_platform(),
        ),
    )

    assert re.search("2 TESTS FATALS", output.stderr, re.DOTALL)
    assert re.search(
        r"test_\d \(bsmr.tests.targets.rules.python.broken.broken_import.TestCase\)",
        output.stderr,
        re.DOTALL,
    )
    assert not re.search("unittest.loader._FailedTest", output.stderr, re.DOTALL)


@bsmr_test(inplace=True, skip_for_os=["windows"])
async def test_python_import_error_with_static_listing_new_provider(bsmr: Bsmr) -> None:
    output = await expect_failure(
        bsmr.test(
            "root//tests/targets/rules/python/broken:broken_with_static_listing_new_adapter",
            get_mode_from_platform(),
        ),
    )
    assert re.search("2 TESTS FATALS", output.stderr, re.DOTALL)
    assert not re.search("unittest.loader._FailedTest", output.stderr, re.DOTALL)
    assert re.search(
        r"test_\d \(bsmr.tests.targets.rules.python.broken.broken_import.TestCase\)",
        output.stderr,
        re.DOTALL,
    )


@bsmr_test(inplace=True, skip_for_os=["windows"])
async def test_python_import_error_with_static_listing_new_provider_bundle(
    bsmr: Bsmr,
) -> None:
    output = await expect_failure(
        bsmr.test(
            "root//tests/targets/rules/python/broken:broken_with_static_listing_new_adapter_bundle",
            get_mode_from_platform(),
        ),
    )
    assert re.search("1 TESTS FATALS", output.stderr, re.DOTALL)
    assert re.search(
        r"bsmr\/tests\/targets\/rules\/python\/broken:broken_with_static_listing_new_adapter_bundle - main",
        output.stderr,
        re.DOTALL,
    )


@bsmr_test(inplace=True)
async def test_tests_dedupe(bsmr: Bsmr) -> None:
    lib_tests = await bsmr.test(
        "root//tests/targets/rules/python/test:lib",
        "root//tests/targets/rules/python/test:tests_for_lib",
        get_mode_from_platform(),
    )
    assert "Pass 1" in remove_ansi_escape_sequences(lib_tests.stderr)


@pytest.mark.parametrize("build_filtered", [(True), (False)])
@bsmr_test(
    inplace=True,
    skip_for_os=["windows"],  # TODO(marwhal): Fix and enable on Windows
)
async def test_label_filtering(bsmr: Bsmr, build_filtered: bool) -> None:
    cmd = ["root//tests/targets/rules/label_test_filtering:"]
    if build_filtered:
        cmd.append("--build-filtered")

    await expect_failure(bsmr.test(*cmd), stderr_regex="1 TESTS FAILED")

    await expect_failure(
        bsmr.test(*cmd, "--exclude", "label-pass"), stderr_regex="1 TESTS FAILED"
    )

    await expect_failure(
        bsmr.test(*cmd, "--include", "label-fail"), stderr_regex="1 TESTS FAILED"
    )

    await expect_failure(
        bsmr.test(*cmd, "--include", "label-fail", "--exclude", "label-pass"),
        stderr_regex="1 TESTS FAILED",
    )

    await expect_failure(
        bsmr.test(
            *cmd,
        ),
        stderr_regex="1 TESTS FAILED",
    )

    await bsmr.test(*cmd, "--include", "label-pass")

    await bsmr.test(*cmd, "--exclude", "label-fail")

    await bsmr.test(*cmd, "--include", "!label-fail")

    await bsmr.test(
        *cmd, "--include", "label-fail", "--exclude", "label-fail", "--always-exclude"
    )

    await bsmr.test(*cmd, "--include", "!label-fail", "label-fail")


@bsmr_test(inplace=True, skip_for_os=MAC_AND_WINDOWS)
async def test_name_filtering(bsmr: Bsmr) -> None:
    await bsmr.test(
        "root//tests/targets/rules/python/test/...", "--", "test_env"
    )

    await expect_failure(
        bsmr.test(
            "root//tests/targets/rules/python/test/...", "--", "test_fail"
        ),
        stderr_regex="1 TESTS FAILED",
    )


@bsmr_test(inplace=True)
async def test_compile_error(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.test(
            "root//tests/targets/compile_error:cpp_test_compile_error",
            get_mode_from_platform(),
        ),
        stderr_regex="#error Compile error.*1 BUILDS FAILED.*NO TESTS RAN",
    )


@bsmr_test(
    inplace=True,
    skip_for_os=["windows"],  # TODO(marwhal): Fix and enable on Windows
)
async def test_cwd(bsmr: Bsmr) -> None:
    await bsmr.test(
        "root//tests/targets/rules/sh_test:test_cwd",
    )


@bsmr_test(
    inplace=True,
    skip_for_os=["windows"],  # TODO(marwhal): Fix and enable on Windows
)
async def test_default_label_filtering(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.test(
            "root//tests/targets/rules/sh_test:test_fail_extended",
            "--",
            "--extended-tests",
        ),
        stderr_regex="1 TESTS FAILED",
    )

    # Ignores it by default
    await bsmr.test(
        "root//tests/targets/rules/sh_test:test_fail_extended",
    )


@bsmr_test(
    inplace=True,
    skip_for_os=["windows"],  # TODO(marwhal): Fix and enable on Windows
)
async def test_stress_runs(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.test(
            "root//tests/targets/rules/sh_test:test_fail",
            "--",
            "--stress-runs",
            "10",
        ),
        stderr_regex="10 TESTS FAILED",
    )


# Not-in-place tests cannot run with deployed bsmr
if not is_deployed_bsmr():

    @bsmr_test(inplace=False, data_dir="testsof")
    @env("BSMR_LOG", "bsmr_test::command=debug")
    async def test_target_compatibility(bsmr: Bsmr) -> None:
        out = await bsmr.test(
            "//...",
            "--target-platforms",
            "//:platform_default_tests",
        )

        assert "target incompatible node" in out.stderr

        await expect_failure(
            bsmr.test(
                "//:foo_extra_test",
                "--target-platforms",
                "//:platform_default_tests",
            ),
            stderr_regex="incompatible",
        )


# TODO(marwhal): Fix and enable on Windows
@bsmr_test(inplace=True, skip_for_os=["windows"])
async def test_external_runner_test_info_options(bsmr: Bsmr) -> None:
    await bsmr.test(
        "root//tests/targets/rules/external_runner_test_info/...",
    )


# TODO(marwhal): Fix and enable on Windows
@bsmr_test(inplace=True, skip_for_os=["windows"])
async def test_allow_tests_on_re(bsmr: Bsmr) -> None:
    await bsmr.test(
        "root//tests/targets/rules/external_runner_test_info/...",
        "--unstable-allow-tests-on-re",
    )


@bsmr_test(inplace=True)
async def test_incompatible_tests_do_not_run_on_re(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.test(
            "root//tests/targets/rules/external_runner_test_info:invalid_test",
            "-c",
            "external_runner_test_info.declare_invalid_test=1",
        ),
        stderr_regex="Trying to execute a `local_only = True` action on remote executor",
    )


@bsmr_test(inplace=True)
@env("TEST_MAKE_IT_FAIL", "1")
async def test_env_var_filtering(bsmr: Bsmr) -> None:
    await bsmr.test(
        "root//tests/targets/rules/python/test:test",
        get_mode_from_platform(),
    )

    await expect_failure(
        bsmr.test(
            "root//tests/targets/rules/python/test:test",
            get_mode_from_platform(),
            "--",
            "--env",
            "TEST_MAKE_IT_FAIL=1",
        ),
        stderr_regex="1 TESTS FAILED",
    )


@bsmr_test(inplace=True, skip_for_os=["windows"])
async def test_prepare_for_local_execution_env_with_env_cli_parameter(
    bsmr: Bsmr, tmp_path: Path
) -> None:
    out = tmp_path / "out"
    await bsmr.test(
        "root//tests/targets/rules/python/test:test",
        "--",
        "--env",
        "EXTRA_VAR=foo",
        "--no-run-output-test-commands-for-fdb",
        str(out),
    )

    with open(out) as f:
        config = json.load(f)

    # Expect python/test:test target to support debugging. Executable field is populated only when debugging is supported.
    assert "debuggers" in config
    assert len(config["debuggers"]) > 0
    assert "executable" in config
    env = config["executable"]["env"]
    assert "PWD" in env
    assert "EXTRA_VAR" in env


# TODO(marwhal): Fix and enable on Windows
@bsmr_test(inplace=True, skip_for_os=["windows"])
@env("EXTRA_VAR", "foo")
async def test_prepare_for_local_execution_env(bsmr: Bsmr, tmp_path: Path) -> None:
    out = tmp_path / "out"
    await bsmr.test(
        "root//tests/targets/rules/python/test:test",
        "--",
        "--no-run-output-test-commands-for-fdb",
        str(out),
    )

    with open(out) as f:
        config = json.load(f)

    # Expect python/test:test target to support debugging. Executable field is populated only when debugging is supported.
    assert "debuggers" in config
    assert len(config["debuggers"]) > 0
    assert "executable" in config
    env = config["executable"]["env"]
    assert "PWD" in env
    assert "EXTRA_VAR" not in env


@bsmr_test(inplace=True)
@env("BSMR_TEST_TPX_USE_TCP", "true")
async def test_tcp(bsmr: Bsmr) -> None:
    await bsmr.test(
        "root//tests/targets/rules/python/test:test",
        get_mode_from_platform(),
    )


@bsmr_test(inplace=True)
async def test_passing_test_names_are_not_shown(bsmr: Bsmr) -> None:
    # Passing test headers are not shown unless we pass --print-passing-details explicitly.
    tests = await bsmr.test(
        "root//tests/targets/rules/python/test:test",
        get_mode_from_platform(),
    )
    assert (
        "Pass: root//tests/targets/rules/python/test:test - test"
        not in tests.stderr
    )


@bsmr_test(inplace=True)
async def test_failing_test_names_are_shown(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.test(
            "root//tests/targets/rules/python/test:test",
            get_mode_from_platform(),
            "--",
            "--env",
            "TEST_ENV=fail",
        ),
        stderr_regex="Fail: root//tests/targets/rules/python/test:test - test",
    )


@bsmr_test(inplace=True)
async def test_no_print_passing_details(bsmr: Bsmr) -> None:
    # Without --print-passing-details, test headers and stdout is NOT displayed.
    tests = await bsmr.test(
        "root//tests/targets/rules/python/test:test",
        get_mode_from_platform(),
    )
    assert (
        "Pass: root//tests/targets/rules/python/test:test - test"
        not in tests.stderr
    )
    assert "TESTED!" not in tests.stderr


@bsmr_test(inplace=True)
async def test_print_passing_details(bsmr: Bsmr) -> None:
    # With --print-passing-details, test headers and stdout is displayed.
    tests = await bsmr.test(
        "root//tests/targets/rules/python/test:test",
        get_mode_from_platform(),
        "--",
        "--print-passing-details",
    )
    assert (
        "Pass: root//tests/targets/rules/python/test:test - test"
        in tests.stderr
    )
    assert "TESTED!" in tests.stderr


@bsmr_test(inplace=True)
async def test_no_no_print_details(bsmr: Bsmr) -> None:
    # Without --no-print-details the stack trace is displayed.
    await expect_failure(
        bsmr.test(
            "root//tests/targets/rules/python/test:test",
            get_mode_from_platform(),
            "--",
            "--env",
            "TEST_ENV=fail",
        ),
        stderr_regex="AssertionError: 41 != 42",
    )


@bsmr_test(inplace=True)
async def test_no_print_details(bsmr: Bsmr) -> None:
    # With --no-print-details the stack trace is not displayed.
    tests = await expect_failure(
        bsmr.test(
            "root//tests/targets/rules/python/test:test",
            "--",
            "--env",
            "TEST_ENV=fail",
            "--no-print-details",
        ),
    )
    assert "AssertionError: 41 != 42" not in tests.stderr


@bsmr_test(inplace=True)
async def test_bundle_sharding(bsmr: Bsmr) -> None:
    tests = await bsmr.test(
        "root//tests/targets/rules/python/test:multi_tests",
        get_mode_from_platform(),
    )
    assert "Pass 4" in tests.stderr


# TODO(marwhal): Fix and enable on Windows
@bsmr_test(inplace=True, skip_for_os=["windows"])
async def test_cancellation(bsmr: Bsmr, tmp_path: Path) -> None:
    """
    This test starts a test that writes its PID to a file then runs for 60
    seconds. We test cancellation by sending a CTRL+C as soon as a test
    starts. We then check that the process exited, and that nothing else
    started (or if anything did, that they stopped).
    """

    # Make sure we are ready to go
    await bsmr.build(
        "root//tests/targets/rules/python/test:cancellation",
        "--build-test-info",
    )

    tests = bsmr.test(
        "root//tests/targets/rules/python/test:cancellation",
        "--",
        "--stress-runs",
        "10",
        "--env",
        "SLOW_DURATION=60",
        "--env",
        f"PIDS={tmp_path}",
    )

    tests = await tests.start()

    for _i in range(30):
        await asyncio.sleep(1)
        pids = os.listdir(tmp_path)
        if pids:
            break
    else:
        raise Exception("Tests never started")

    tests.send_signal(signal.SIGINT)
    await tests.communicate()  # Wait for the command to exit

    # Give stuff time to settle, PIDS don't necessarily disappear
    # instantly. Also, verify that we are not starting more tests.
    await asyncio.sleep(5)

    # At this point, nothing should be alive.
    pids = os.listdir(tmp_path)
    for pid in pids:
        try:
            os.kill(int(pid), 0)
        except OSError:
            pass
        else:
            raise Exception(f"PID existed: {pid}")


@bsmr_test(inplace=True, skip_for_os=["windows"])
async def test_cancellation_on_re(bsmr: Bsmr) -> None:
    """
    This test starts a test on RE, waits for it to start, cancels, then starts
    again and verifies we don't wait for the test to finish.
    """

    # Make sure we are ready to go
    await bsmr.build(
        "root//tests/targets/rules/python/test:cancellation",
        "--build-test-info",
    )

    tests = bsmr.test(
        "root//tests/targets/rules/python/test:cancellation",
        "--unstable-force-tests-on-re",
        "--remote-only",
        "--no-remote-cache",
        "--",
        "--env",
        "SLOW_DURATION=60",
        "--env",
        "PIDS=/tmp",
    )

    tests = await tests.start()

    async def has_started() -> bool:
        try:
            stdout = (await bsmr.log("what-ran")).stdout
        except BsmrException as e:
            # The log is truncated here so this can exit non-zero.
            stdout = e.stdout

        # what-ran returns things that started
        return "test.run" in stdout

    for _i in range(30):
        await asyncio.sleep(1)
        if await has_started():
            break
    else:
        raise Exception("Tests never started")

    tests.send_signal(signal.SIGINT)
    await tests.communicate()  # Wait for the command to exit

    # Run a command that cannot execute concurrerntly and check it does not
    # take 60 seconds to run, which means we went idle.
    await asyncio.wait_for(bsmr.audit_config("-c", "foo.bar=True"), timeout=10)


@bsmr_test(inplace=True, skip_for_os=["windows"])
async def test_timeout_local(bsmr: Bsmr) -> None:
    result = await expect_failure(
        bsmr.test(
            "root//tests/targets/rules/python/test:timeout",
            "--local-only",
            "--no-remote-cache",
            "--",
            "--env",
            "SLOW_DURATION=60",
            "--timeout=5",
        ),
        stderr_regex="Timeout: root//tests/targets/rules/python/test:timeout",
    )
    assert "1 TESTS TIMED OUT" in result.stderr


@bsmr_test(inplace=True, skip_for_os=["windows"])
async def test_timeout_re(bsmr: Bsmr) -> None:
    result = await expect_failure(
        bsmr.test(
            "root//tests/targets/rules/python/test:timeout",
            "--unstable-allow-all-tests-on-re",
            "--remote-only",
            "--no-remote-cache",
            "--",
            "--env",
            "SLOW_DURATION=60",
            "--timeout=5",
        ),
        stderr_regex="Timeout: root//tests/targets/rules/python/test:timeout",
    )
    assert "1 TESTS TIMED OUT" in result.stderr


@bsmr_test(inplace=True, skip_for_os=["windows"])
async def test_timeout_and_failure_local(bsmr: Bsmr) -> None:
    result = await expect_failure(
        bsmr.test(
            "root//tests/targets/rules/python/test:timeout_and_fail",
            "--local-only",
            "--no-remote-cache",
            "--",
            "--env",
            "SLOW_DURATION=60",
            "--timeout=5",
        ),
        stderr_regex="1 TESTS FAILED.*1 TESTS TIMED OUT",
    )
    stderr = remove_ansi_escape_sequences(result.stderr)
    assert (
        "Tests finished: Pass 0. Fail 1. Timeout 1. Fatal 0. Skip 0. Omit 0. Infra Failure 0. Build failure 0"
        in stderr
    )


if not is_deployed_bsmr():

    @bsmr_test(inplace=True, skip_for_os=["windows"])
    async def test_overall_timeout(bsmr: Bsmr) -> None:
        """
        If an overall timeout is set, we expect that to result in OMITs
        reported in Tpx, and Tpx does not set an error status for that.

        We're OK with that, we will report how many OMITs there were.
        The caller is expected to be aware of how this feature works.
        """
        bsmr.test(
            "root//tests/targets/rules/python/test:timeout",
            "--local-only",
            "--no-remote-cache",
            "--overall-timeout",
            "5s",
            "--",
            "--env",
            "SLOW_DURATION=60",
        )


@bsmr_test(inplace=True, skip_for_os=["windows"])
@pytest.mark.parametrize(
    "test",
    ["requires_env", "requires_env_location"],
)
async def test_test_env(bsmr: Bsmr, test: str) -> None:
    test = f"root//tests/targets/rules/sh_test:{test}"

    await bsmr.test(test)

    # Check run also works. Note that those tests run from `fbcode` by default
    # so no chdir needed here.
    await bsmr.run(test)


@bsmr_test(inplace=True, skip_for_os=["windows"])
async def test_exit_code(bsmr: Bsmr) -> None:
    result = await expect_failure(
        bsmr.test("root//tests/targets/rules/sh_test:test_fail")
    )
    assert result.process.returncode == 32
    result = await expect_failure(bsmr.test("not//a/real:target"))
    assert result.process.returncode == ExitCodeV2.USER_ERROR.value


@bsmr_test(inplace=True, skip_for_os=["windows"])
async def test_skip_missing_targets(bsmr: Bsmr) -> None:
    await expect_failure(
        bsmr.test("root//tests/targets/rules/python/test:not_a_thing"),
        stderr_regex="Unknown target `not_a_thing`",
    )

    res = await bsmr.test(
        "root//tests/targets/rules/python/test:not_a_thing",
        "--skip-missing-targets",
    )

    assert "Skipped 1 missing targets:" in res.stderr


@bsmr_test(inplace=True, skip_for_os=["darwin", "windows"])
async def test_test_worker(bsmr: Bsmr) -> None:
    worker_args = [
        "-c",
        "build.use_persistent_workers=True",
        "--local-only",
        "--no-remote-cache",
    ]
    await bsmr.test(
        *worker_args, "root//tests/targets/rules/worker_grpc:worker_test"
    )


@bsmr_test(inplace=True, write_invocation_record=True)
@env("TEST_MAKE_IT_FAIL", "1")
async def test_failed_tests_has_error_category(bsmr: Bsmr) -> None:
    res = await expect_failure(
        bsmr.test(
            "root//tests/targets/rules/python/test:test",
            get_mode_from_platform(),
            "--",
            "--env",
            "TEST_MAKE_IT_FAIL=1",
        ),
        stderr_regex="1 TESTS FAILED",
    )
    record = res.invocation_record()
    errors = record["errors"]

    assert len(errors) == 1
    assert errors[0]["category"] == "USER"
    if not is_deployed_bsmr():
        assert errors[0]["category_key"] == "TEST_FAILED"
