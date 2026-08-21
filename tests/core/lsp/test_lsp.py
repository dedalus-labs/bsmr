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
import signal
from pathlib import Path
from typing import Any, Optional

import pytest
from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.api.bsmr_result import BsmrException
from bsmr.tests.e2e_util.api.fixtures import Fixture, Span
from bsmr.tests.e2e_util.api.lsp import LSPResponseError
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test, env
from bsmr.tests.e2e_util.helper.utils import daemon_is_alive


def _assert_range(range: dict[str, Any], expected: Optional[Span]) -> None:
    """Assert that this Span is equal to an LSP range dict"""
    if expected is None:
        expected = Span(0, 0, 0, 0)
    assert range["start"]["line"] == expected.start_line
    assert range["start"]["character"] == expected.start_col
    assert range["end"]["line"] == expected.end_line
    assert range["end"]["character"] == expected.end_col


def _assert_uris(actual: str, expected: str) -> None:
    if os.name == "nt":
        # Windows file paths are case-insensitive, and the LSP returns the drive identifier in upper-case.
        # Windows also allows paths to use forward and backward slashes interchangeably.
        # Normalize the paths only on Windows to avoid flakiness.
        assert actual.replace("\\", "/").replace("%3A", ":").lower() == expected.lower()
    else:
        assert actual == expected


def _assert_goto_result(
    res: list[dict[str, Any]],
    expected_src: Span,
    expected_dest_path: Path,
    expected_dest: Optional[Span],
) -> None:
    assert len(res) == 1
    _assert_range(res[0]["originSelectionRange"], expected_src)
    _assert_range(res[0]["targetRange"], expected_dest)
    _assert_range(res[0]["targetSelectionRange"], expected_dest)
    _assert_uris(res[0]["targetUri"], expected_dest_path.as_uri())


def fixture(bsmr: Bsmr, path: Path) -> Fixture:
    abs_path = bsmr.cwd / path
    fixture = Fixture(abs_path.read_text())
    abs_path.write_text(fixture.content)
    return fixture


async def _wait_for_exit(process: asyncio.subprocess.Process, timeout: float) -> bool:
    try:
        await asyncio.wait_for(process.wait(), timeout=timeout)
        return True
    except TimeoutError:
        return False


async def _kill_if_alive(process: asyncio.subprocess.Process) -> None:
    if process.returncode is not None:
        return

    process.kill()
    await asyncio.wait_for(process.wait(), timeout=30)


async def _wait_for_file_to_contain(
    path: Path,
    substring: str,
    timeout: float,
) -> bool:
    deadline = asyncio.get_running_loop().time() + timeout
    while asyncio.get_running_loop().time() < deadline:
        if path.exists() and substring in path.read_text():
            return True
        await asyncio.sleep(1)
    return False


def _active_commands_snapshot_has_command(
    msg: dict[str, Any],
    command_name: str,
) -> bool:
    snapshot = msg.get("response", {}).get("ActiveCommandsSnapshot")
    if snapshot is None:
        return False

    return any(
        command_name in command["argv"] for command in snapshot["active_commands"]
    )


async def _wait_for_active_command_state(
    subscribe: Any,
    command_name: str,
    present: bool,
    timeout: float,
) -> bool:
    deadline = asyncio.get_running_loop().time() + timeout
    while True:
        remaining = deadline - asyncio.get_running_loop().time()
        if remaining <= 0:
            return False

        try:
            msg = await asyncio.wait_for(subscribe.read_message(), timeout=remaining)
        except TimeoutError:
            return False

        if _active_commands_snapshot_has_command(msg, command_name) == present:
            return True


@bsmr_test()
async def test_lsp_starts(bsmr: Bsmr) -> None:
    async with await bsmr.lsp() as lsp:
        # Will fail if the initialize response is not received
        await lsp.init_connection()


@bsmr_test()
async def test_lsp_stdin_eof_clears_server_command(
    bsmr: Bsmr,
) -> None:
    try:
        async with await bsmr.subscribe("--active-commands") as subscribe:
            lsp = await bsmr.lsp()
            try:
                await lsp.init_connection()
                assert await _wait_for_active_command_state(
                    subscribe, "lsp", present=True, timeout=10
                )

                assert lsp.process.stdin is not None
                lsp.process.stdin.close()

                exited = await _wait_for_exit(lsp.process, timeout=10)
                assert exited
                assert lsp.process.returncode is not None

                assert await _wait_for_active_command_state(
                    subscribe, "lsp", present=False, timeout=10
                )
            finally:
                await _kill_if_alive(lsp.process)
    finally:
        await bsmr.kill()


@bsmr_test()
@env("BSMR_TESTING_INACTIVITY_TIMEOUT", "true")
async def test_lsp_does_not_exit_when_daemon_times_out(bsmr: Bsmr) -> None:
    await bsmr.server()
    status = await bsmr.status()
    pid = json.loads(status.stdout)["process_info"]["pid"]
    daemon_dir = await bsmr.get_daemon_dir()
    daemon_stderr = daemon_dir / "bsmrd.stderr"

    lsp = await bsmr.lsp()
    try:
        exited = await _wait_for_exit(lsp.process, timeout=10)
        assert not exited
        saw_inactivity_timeout = await _wait_for_file_to_contain(
            daemon_stderr,
            "inactivity timeout elapsed",
            timeout=20,
        )
        assert saw_inactivity_timeout
        assert daemon_is_alive(pid)
    finally:
        await _kill_if_alive(lsp.process)


@bsmr_test(skip_for_os=["windows"])
@env("BSMR_TESTING_INACTIVITY_TIMEOUT", "true")
@env("BSMRD_STARTUP_TIMEOUT", "90")
async def test_lsp_daemon_inactivity_shutdown_currently_times_out_before_recovering_different_user_version(
    bsmr: Bsmr,
) -> None:
    await bsmr.server()
    status = await bsmr.status()
    original_pid = json.loads(status.stdout)["process_info"]["pid"]
    daemon_dir = await bsmr.get_daemon_dir()
    daemon_stderr = daemon_dir / "bsmrd.stderr"
    daemon_info = daemon_dir / "bsmrd.info"

    lsp = await bsmr.lsp()
    try:
        exited = await _wait_for_exit(lsp.process, timeout=10)
        assert not exited

        saw_inactivity_timeout = await _wait_for_file_to_contain(
            daemon_stderr,
            "inactivity timeout elapsed",
            timeout=20,
        )
        assert saw_inactivity_timeout
        assert daemon_is_alive(original_pid)

        info = json.loads(daemon_info.read_text())
        info["version"] = "different-version"
        daemon_info.write_text(json.dumps(info))

        start = asyncio.get_running_loop().time()
        with pytest.raises(BsmrException) as exc:
            await bsmr.server()
        elapsed = asyncio.get_running_loop().time() - start

        assert elapsed >= 90
        assert "Failed to connect to bsmr daemon." in exc.value.stderr
        assert "version: different-version" in exc.value.stderr
    finally:
        await _kill_if_alive(lsp.process)


@bsmr_test()
async def test_lsp_exits_when_daemon_disappears(bsmr: Bsmr) -> None:
    await bsmr.server()

    lsp = await bsmr.lsp()
    try:
        await lsp.init_connection()
        await bsmr.kill()

        exited = await _wait_for_exit(lsp.process, timeout=10)
        assert exited
        assert lsp.process.returncode is not None
    finally:
        await _kill_if_alive(lsp.process)


@bsmr_test()
@env("BSMR_TESTING_INACTIVITY_TIMEOUT", "true")
async def test_lsp_requests_keep_daemon_alive(bsmr: Bsmr) -> None:
    async with await bsmr.lsp() as lsp:
        await lsp.init_connection()
        daemon_info = await bsmr.get_daemon_dir() / "bsmrd.info"
        pid = json.loads(daemon_info.read_text())["pid"]

        for _ in range(6):
            await asyncio.sleep(0.2)
            await lsp.open_file(Path("clean_lint.bzl"))

        assert json.loads(daemon_info.read_text())["pid"] == pid
        assert lsp.process.returncode is None


@bsmr_test(skip_for_os=["windows"])
async def test_lsp_exits_when_daemon_is_killed(bsmr: Bsmr) -> None:
    await bsmr.server()
    status = await bsmr.status()
    pid = json.loads(status.stdout)["process_info"]["pid"]

    lsp = await bsmr.lsp()
    try:
        await lsp.init_connection()
        os.kill(pid, signal.SIGKILL)

        exited = await _wait_for_exit(lsp.process, timeout=8)
        assert exited
        assert lsp.process.returncode is not None
    finally:
        await _kill_if_alive(lsp.process)


@bsmr_test()
async def test_lints_on_open(bsmr: Bsmr) -> None:
    async with await bsmr.lsp() as lsp:
        await lsp.init_connection()
        diags = await lsp.open_file(Path("clean_lint.bzl"))
        assert diags is not None
        assert len(diags["diagnostics"]) == 0

        diags = await lsp.open_file(Path("bad_syntax.bzl"))
        assert diags is not None
        assert len(diags["diagnostics"]) == 1


@bsmr_test()
async def test_goto_definition(bsmr: Bsmr) -> None:
    src_targets_path = Path("dir/TARGETS.fixture")
    dest_targets_path = Path("cell/sub/TARGETS.fixture")
    dest_bzl_path = Path("cell/sub/defs.bzl")

    src_targets = fixture(bsmr, src_targets_path)
    dest_targets = fixture(bsmr, dest_targets_path)
    dest_bzl = fixture(bsmr, dest_bzl_path)

    async with await bsmr.lsp() as lsp:
        await lsp.init_connection()
        diags = await lsp.open_file(src_targets_path)
        # pyrefly: ignore [unsupported-operation]
        assert len(diags["diagnostics"]) == 0

        res = await lsp.goto_definition(
            src_targets_path,
            src_targets.start_line("load_click"),
            src_targets.start_col("load_click"),
        )
        _assert_goto_result(
            # pyrefly: ignore [bad-argument-type]
            res,
            src_targets.spans["load"],
            bsmr.cwd / dest_bzl_path,
            None,
        )

        res = await lsp.goto_definition(
            src_targets_path,
            src_targets.start_line("dummy_click"),
            src_targets.start_col("dummy_click"),
        )
        _assert_goto_result(
            # pyrefly: ignore [bad-argument-type]
            res,
            src_targets.spans["dummy"],
            bsmr.cwd / dest_bzl_path,
            dest_bzl.spans["rule"],
        )

        res = await lsp.goto_definition(
            src_targets_path,
            src_targets.start_line("missing_click"),
            src_targets.start_col("missing_click"),
        )
        # pyrefly: ignore [bad-argument-type]
        assert len(res) == 0

        res = await lsp.goto_definition(
            src_targets_path,
            src_targets.start_line("missing_foo_click"),
            src_targets.start_col("missing_foo_click"),
        )
        _assert_goto_result(
            # pyrefly: ignore [bad-argument-type]
            res,
            src_targets.spans["missing_foo"],
            bsmr.cwd / dest_targets_path,
            None,
        )

        res = await lsp.goto_definition(
            src_targets_path,
            src_targets.start_line("rule_click"),
            src_targets.start_col("rule_click"),
        )
        _assert_goto_result(
            # pyrefly: ignore [bad-argument-type]
            res,
            src_targets.spans["rule"],
            bsmr.cwd / dest_bzl_path,
            dest_bzl.spans["rule"],
        )

        res = await lsp.goto_definition(
            src_targets_path,
            src_targets.start_line("baz_click"),
            src_targets.start_col("baz_click"),
        )
        _assert_goto_result(
            # pyrefly: ignore [bad-argument-type]
            res,
            src_targets.spans["baz"],
            bsmr.cwd / dest_targets_path,
            dest_targets.spans["baz"],
        )


@bsmr_test()
async def test_returns_file_contents_for_starlark_types(bsmr: Bsmr) -> None:
    async with await bsmr.lsp() as lsp:
        await lsp.init_connection()

        res = await lsp.file_contents("starlark:/native/DefaultInfo.bzl")
        # pyrefly: ignore [unsupported-operation]
        assert res["contents"] is not None

        res = await lsp.file_contents("starlark:/native/NonExistent.bzl")
        # pyrefly: ignore [unsupported-operation]
        assert res["contents"] is None

        with pytest.raises(LSPResponseError):
            await lsp.file_contents((lsp.cwd / ".bsmr").as_uri())


@bsmr_test()
async def test_goto_definition_for_globals(bsmr: Bsmr) -> None:
    globals_bzl_path = Path("globals.bzl")

    globals_bzl = fixture(bsmr, globals_bzl_path)
    async with await bsmr.lsp() as lsp:
        await lsp.init_connection()
        diags = await lsp.open_file(globals_bzl_path)
        # pyrefly: ignore [unsupported-operation]
        assert len(diags["diagnostics"]) == 0

        res = await lsp.goto_definition(
            globals_bzl_path,
            globals_bzl.start_line("func2_click"),
            globals_bzl.start_col("func2_click"),
        )

        # pyrefly: ignore [bad-argument-type]
        assert len(res) == 1
        # pyrefly: ignore [unsupported-operation]
        _assert_range(res[0]["originSelectionRange"], globals_bzl.spans["func2"])
        # pyrefly: ignore [unsupported-operation]
        assert res[0]["targetRange"]["start"]["line"] != 0
        # pyrefly: ignore [unsupported-operation]
        assert res[0]["targetSelectionRange"]["start"]["line"] != 0
        _assert_uris(
            # pyrefly: ignore [unsupported-operation]
            res[0]["targetUri"],
            (bsmr.cwd / "prelude" / "prelude.bzl").as_uri(),
        )

        res = await lsp.goto_definition(
            globals_bzl_path,
            globals_bzl.start_line("info_click"),
            globals_bzl.start_col("info_click"),
        )

        # pyrefly: ignore [bad-argument-type]
        assert len(res) == 1
        # pyrefly: ignore [unsupported-operation]
        _assert_range(res[0]["originSelectionRange"], globals_bzl.spans["info"])
        # pyrefly: ignore [unsupported-operation]
        _assert_uris(res[0]["targetUri"], "starlark:/native/DefaultInfo.bzl")

        res = await lsp.goto_definition(
            globals_bzl_path,
            globals_bzl.start_line("invalid_click"),
            globals_bzl.start_col("invalid_click"),
        )
        # pyrefly: ignore [bad-argument-type]
        assert len(res) == 0


@bsmr_test()
async def test_supports_bxl_files(bsmr: Bsmr) -> None:
    src_bxl_path = Path("query.bxl")

    src_bxl = fixture(bsmr, src_bxl_path)

    async with await bsmr.lsp() as lsp:
        await lsp.init_connection()
        diags = await lsp.open_file(src_bxl_path)
        # pyrefly: ignore [unsupported-operation]
        assert len(diags["diagnostics"]) == 0

        res = await lsp.goto_definition(
            src_bxl_path,
            src_bxl.start_line("foo_click"),
            src_bxl.start_col("foo_click"),
        )
        _assert_goto_result(
            # pyrefly: ignore [bad-argument-type]
            res,
            src_bxl.spans["foo"],
            bsmr.cwd / src_bxl_path,
            src_bxl.spans["dest_foo"],
        )

        res = await lsp.goto_definition(
            src_bxl_path,
            src_bxl.start_line("f_click"),
            src_bxl.start_col("f_click"),
        )
        _assert_goto_result(
            # pyrefly: ignore [bad-argument-type]
            res,
            src_bxl.spans["f"],
            bsmr.cwd / src_bxl_path,
            src_bxl.spans["dest_f"],
        )
