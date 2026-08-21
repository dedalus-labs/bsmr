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
import shutil
from os.path import exists, islink
from pathlib import Path

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test, env
from bsmr.tests.e2e_util.helper.utils import read_timestamps


def _setup_sandbox(bsmr: Bsmr) -> None:
    """Copy pre-built installer binaries into the sandbox project directory."""
    installer_bin = os.environ["INSTALLER_BIN"]
    shutil.copy2(installer_bin, bsmr.cwd / "installer_bin")
    os.chmod(bsmr.cwd / "installer_bin", 0o755)

    forwarded_params_bin = os.environ["FORWARDED_PARAMS_INSTALLER_BIN"]
    shutil.copy2(forwarded_params_bin, bsmr.cwd / "forwarded_params_installer_bin")
    os.chmod(bsmr.cwd / "forwarded_params_installer_bin", 0o755)

    extra_args_validator_bin = os.environ["EXTRA_ARGS_VALIDATOR_BIN"]
    shutil.copy2(extra_args_validator_bin, bsmr.cwd / "extra_args_validator_bin")
    os.chmod(bsmr.cwd / "extra_args_validator_bin", 0o755)

    early_exit_installer_bin = os.environ["EARLY_EXIT_INSTALLER_BIN"]
    shutil.copy2(early_exit_installer_bin, bsmr.cwd / "early_exit_installer_bin")
    os.chmod(bsmr.cwd / "early_exit_installer_bin", 0o755)

    # Create etc_hosts as a symlink to /etc/hosts (tests symlink resolution by rsync -aL)
    os.symlink("/etc/hosts", bsmr.cwd / "etc_hosts")


@bsmr_test()
async def test_success_install(bsmr: Bsmr, tmp_path: Path) -> None:
    _setup_sandbox(bsmr)
    tmp_dir = tmp_path / "install_test"
    tmp_dir.mkdir()
    args = ["--dst", f"{tmp_dir}/"]
    await bsmr.install("root//:installer_test", "--", *args)
    assert exists(f"{tmp_dir}/artifact_a")
    assert exists(f"{tmp_dir}/artifact_b")
    assert exists(f"{tmp_dir}/etc_hosts")
    assert not islink(f"{tmp_dir}/etc_hosts")


@bsmr_test(write_invocation_record=True)
@env("BSMR_LOG", "bsmr_server_commands::commands::install=debug")
async def test_install_logging(bsmr: Bsmr, tmp_path: Path) -> None:
    _setup_sandbox(bsmr)
    tmp_dir = tmp_path / "install_test"
    tmp_dir.mkdir()
    args = ["--dst", f"{tmp_dir}/"]
    args += ["--delay", "1"]
    res = await bsmr.install(
        "root//:installer_test",
        "--",
        *args,
    )
    invocation_record = res.invocation_record()

    cmd_start_ts = (
        await read_timestamps(bsmr, "Event", "data", "SpanStart", "data", "Command")
    )[0]
    action_end_timestamps = await read_timestamps(
        bsmr, "Event", "data", "SpanEnd", "data", "ActionExecution"
    )

    install_duration_ms = invocation_record["install_duration_us"] / 1000
    cmd_duration_ms = invocation_record["command_duration_us"] / 1000

    # Check that installing takes at least as long as the added delay.
    assert install_duration_ms > 1 * 1000
    # Check that the we aren't double counting any time between install and
    # building the last action (only meaningful when there are build actions).
    if action_end_timestamps:
        last_action_end_ts = action_end_timestamps[-1]
        time_to_last_action_ms = last_action_end_ts - cmd_start_ts
        assert time_to_last_action_ms + install_duration_ms < cmd_duration_ms

    assert invocation_record["install_device_metadata"] == [
        {"entry": [{"key": "version", "value": "1"}]}
    ]


@bsmr_test(write_invocation_record=True)
async def test_install_logs_target_rule_type_names(bsmr: Bsmr, tmp_path: Path) -> None:
    _setup_sandbox(bsmr)
    tmp_dir = tmp_path / "install_test"
    tmp_dir.mkdir()
    args = ["--dst", f"{tmp_dir}/"]
    res = await bsmr.install("root//:installer_test", "--", *args)
    record = res.invocation_record()
    assert record["target_rule_type_names"] == ["installer"]


@bsmr_test()
@env("BSMR_INSTALLER_SEND_TIMEOUT_S", "1")
async def test_send_file_timeout(bsmr: Bsmr, tmp_path: Path) -> None:
    _setup_sandbox(bsmr)
    await expect_failure(
        bsmr.install(
            "root//:installer_single_artifact",
            "--",
            "--delay",
            "30",
        ),
        stderr_regex=r"Timed out after 1s waiting for installer to process artifact_a",
    )


@bsmr_test(write_invocation_record=True)
async def test_artifact_fails_to_install(bsmr: Bsmr) -> None:
    _setup_sandbox(bsmr)
    res = await expect_failure(
        bsmr.install(
            "root//:installer_server_sends_error",
        ),
        stderr_regex=r"Interaction with installer failed",
    )
    record = res.invocation_record()
    errors = record["errors"]
    assert len(errors) == 1
    error = errors[0]
    assert "Mocking failing to install" in error["message"]
    assert error["category"] == "INFRA"
    assert "INSTALLER_TAG" in error["category_key"]

    install_duration_ms = record["install_duration_us"] / 1000

    assert install_duration_ms > 0
    assert record["install_device_metadata"] == [
        {"entry": [{"key": "version", "value": "1"}]}
    ]


@bsmr_test(write_invocation_record=True)
async def test_fail_to_build_artifact(bsmr: Bsmr) -> None:
    _setup_sandbox(bsmr)
    res = await expect_failure(
        bsmr.install(
            "root//:bad_artifacts",
        ),
        stderr_regex=r"Failed to build",
    )
    record = res.invocation_record()
    errors = record["errors"]
    assert len(errors) == 1


@bsmr_test(write_invocation_record=True)
async def test_install_id_mismatch(bsmr: Bsmr) -> None:
    _setup_sandbox(bsmr)
    res = await expect_failure(
        bsmr.install(
            "root//:installer_server_sends_wrong_install_info_response",
        ),
        stderr_regex=r"doesn't match with the sent one",
    )
    record = res.invocation_record()
    errors = record["errors"]
    assert len(errors) == 1


@bsmr_test(write_invocation_record=True)
async def test_installer_needs_forwarded_params(bsmr: Bsmr) -> None:
    _setup_sandbox(bsmr)
    res = await expect_failure(
        bsmr.install(
            "root//:installer_server_requires_forwarded_params",
        ),
        stderr_regex=r"-r_-e_-d_-s_-x_-a_-i_-w_-u_-k_must_be_passed_to_installer",
    )
    record = res.invocation_record()
    errors = record["errors"]
    assert len(errors) == 1


@bsmr_test()
async def test_install_forwards_params(bsmr: Bsmr) -> None:
    _setup_sandbox(bsmr)
    await bsmr.install(
        "-r",
        "-e",
        "-d",
        "-s",
        "serial",
        "-x",
        "-a",
        "activity",
        "-i",
        "intent",
        "-w",
        "-u",
        "-k",
        "root//:installer_server_requires_forwarded_params",
    )


@bsmr_test()
async def test_install_forwards_params_long_form(bsmr: Bsmr) -> None:
    _setup_sandbox(bsmr)
    await bsmr.install(
        "--run",
        "--emulator",
        "--device",
        "--serial",
        "serial",
        "--all-devices",
        "--activity",
        "activity",
        "--intent-uri",
        "intent",
        "--wait-for-debugger",
        "--uninstall",
        "--keep",
        "root//:installer_server_requires_forwarded_params",
    )


@bsmr_test()
async def test_install_extra_args_ordered_after_builtin_args(
    bsmr: Bsmr,
) -> None:
    _setup_sandbox(bsmr)
    await bsmr.install(
        "-r",
        "root//:installer_validates_extra_args_order",
        "--",
        "--",
        "--expected-extra-arg",
    )


@bsmr_test(write_invocation_record=True)
async def test_fail_to_build_installer(bsmr: Bsmr) -> None:
    _setup_sandbox(bsmr)
    res = await expect_failure(
        bsmr.install(
            "root//:bad_installer_target",
        ),
        stderr_regex=r"Failed to build installer",
    )
    record = res.invocation_record()
    errors = record["errors"]
    assert len(errors) == 1


@bsmr_test()
async def test_installer_early_exit(bsmr: Bsmr) -> None:
    _setup_sandbox(bsmr)
    await expect_failure(
        bsmr.install(
            "root//:installer_early_exit",
        ),
        stderr_regex=r"Installer process exited with status",
    )
