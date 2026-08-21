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
from pathlib import Path
from tempfile import TemporaryDirectory

import pytest
from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.api.bsmr_result import BsmrException, BsmrResult
from bsmr.tests.e2e_util.api.process import Process
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test
from bsmr.tests.e2e_util.helper.golden import golden


PROFILERS = [
    "heap-flame-allocated",
    "heap-flame-retained",
    "heap-summary-allocated",
    "heap-summary-retained",
    "time-flame",
    "statement",
    "bytecode",
    "bytecode-pairs",
    "typecheck",
    "coverage",
]


@bsmr_test()
@pytest.mark.parametrize(
    "profiler",
    PROFILERS,
)
async def test_profile_analysis_for_self_transition(
    bsmr: Bsmr, tmp_path: Path, profiler: str
) -> None:
    file_path = tmp_path / "profile"

    await bsmr.profile(
        "analysis",
        "--target-platforms=//self_transition:p",
        "--mode",
        profiler,
        "//self_transition:zzz",
        "--output",
        str(file_path),
    )

    assert os.path.exists(file_path)


@bsmr_test()
@pytest.mark.parametrize(
    "profiler",
    PROFILERS,
)
async def test_profile_analysis_last(bsmr: Bsmr, tmp_path: Path, profiler: str) -> None:
    file_path = tmp_path / "profile"

    await bsmr.profile(
        "analysis",
        "--mode",
        profiler,
        "//simple:test",
        "--output",
        str(file_path),
    )

    assert os.path.exists(file_path)


@bsmr_test()
@pytest.mark.parametrize(
    "profiler",
    PROFILERS,
)
async def test_profile_analysis_recursive(
    bsmr: Bsmr, tmp_path: Path, profiler: str
) -> None:
    file_path = tmp_path / "profile"

    command = bsmr.profile(
        "analysis",
        "--mode",
        profiler,
        "//simple:test",
        "--output",
        str(file_path),
        "--recursive",
    )
    await assert_flame_outputs(command, file_path, profiler)


@bsmr_test()
@pytest.mark.parametrize(
    "profiler",
    PROFILERS,
)
async def test_profile_analysis_recursive_transition(
    bsmr: Bsmr, tmp_path: Path, profiler: str
) -> None:
    file_path = tmp_path / "profile"

    command = bsmr.profile(
        "analysis",
        "--mode",
        profiler,
        "//recursive_transition:ccc",
        "--output",
        str(file_path),
        "--recursive",
    )

    await assert_flame_outputs(command, file_path, profiler)


@bsmr_test()
@pytest.mark.parametrize(
    "profiler",
    PROFILERS,
)
async def test_profile_loading_last(bsmr: Bsmr, tmp_path: Path, profiler: str) -> None:
    file_path = tmp_path / "profile"

    command = bsmr.profile(
        "loading",
        "--mode",
        profiler,
        "//simple:",
        "--output",
        str(file_path),
    )

    await _assertions_for_profile_without_frozen_module(command, file_path, profiler)


@bsmr_test()
@pytest.mark.parametrize(
    "profiler",
    PROFILERS,
)
async def test_query_profile(bsmr: Bsmr, tmp_path: Path, profiler: str) -> None:
    file_path = tmp_path / "profile"

    command = bsmr.cquery(
        "--profile-mode",
        profiler,
        "deps(//query/a:a)",
        "--profile-output",
        str(file_path),
    )

    await _assertions_for_profile_without_frozen_module(command, file_path, profiler)

    if not profiler.endswith("-retained"):
        with open(bsmr.cwd / file_path / "targets.txt", "r") as f:
            lines = [x.rstrip() for x in sorted(f.readlines())]
            assert [
                "load/root//query/a",
                "load/root//query/b",
            ] == lines
    else:
        assert not os.path.exists(bsmr.cwd / file_path)


@bsmr_test()
async def test_profile_loading_last_single_target(bsmr: Bsmr, tmp_path: Path) -> None:
    file_path = tmp_path / "profile"

    profiler = "statement"

    command = bsmr.profile(
        "loading",
        "--mode",
        profiler,
        "//simple:a",
        "--output",
        str(file_path),
    )

    await _assertions_for_profile_without_frozen_module(command, file_path, profiler)


@bsmr_test()
@pytest.mark.parametrize(
    "profiler",
    PROFILERS,
)
@pytest.mark.parametrize(
    "recursive",
    [True, False],
)
async def test_profile_analysis_pattern(
    bsmr: Bsmr, tmp_path: Path, profiler: str, recursive: bool
) -> None:
    file_path = tmp_path / "profile"

    command = bsmr.profile(
        "analysis",
        "--mode",
        profiler,
        "//simple/...",  # We test this.
        "--output",
        str(file_path),
        *(["--recursive"] if recursive else []),
    )

    await assert_flame_outputs(command, file_path, profiler)


@bsmr_test()
@pytest.mark.parametrize(
    "profiler",
    PROFILERS,
)
async def test_profile_loading_recursive(
    bsmr: Bsmr, tmp_path: Path, profiler: str
) -> None:
    file_path = tmp_path / "profile"

    command = bsmr.profile(
        "loading",
        "--mode",
        profiler,
        "//simple:",
        "--output",
        str(file_path),
        "--recursive",
    )

    await expect_failure(
        command,
        stderr_regex="Recursive profiling is not supported for loading profiling",
    )


@bsmr_test()
@pytest.mark.parametrize(
    "profiler",
    PROFILERS,
)
async def test_profile_bxl_with_actions(
    bsmr: Bsmr, tmp_path: Path, profiler: str
) -> None:
    file_path = tmp_path / "profile"

    command = bsmr.profile(
        "bxl",
        "--mode",
        profiler,
        "//bxl/profile.bxl:profile_with_actions",
        "--output",
        str(file_path),
    )

    await assert_flame_outputs(command, file_path, profiler)


@bsmr_test()
@pytest.mark.parametrize(
    "profiler",
    PROFILERS,
)
async def test_profile_bxl_without_actions(
    bsmr: Bsmr, tmp_path: Path, profiler: str
) -> None:
    file_path = tmp_path / "profile"

    command = bsmr.profile(
        "bxl",
        "--mode",
        profiler,
        "//bxl/profile.bxl:profile_without_actions",
        "--output",
        str(file_path),
    )

    await assert_flame_outputs(command, file_path, profiler)


@bsmr_test(setup_eden=True)
async def test_profile_no_daemon(
    bsmr: Bsmr,
    tmp_path: Path,
) -> None:
    file_path = tmp_path / "profile"

    command = await bsmr.profile(
        "loading",
        "--mode",
        "statement",
        "//simple:",
        "--output",
        str(file_path),
        "--no-bsmrd",
    )

    assert "Total retained bytes:" in command.stdout
    assert os.path.exists(file_path)


@bsmr_test()
async def test_profile_loading_recursive_target_pattern(
    bsmr: Bsmr, tmp_path: Path
) -> None:
    file_path = tmp_path / "profile"
    profiler = "time-flame"

    command = bsmr.profile(
        "loading",
        "--mode",
        profiler,
        "//simple/...",
        "--output",
        str(file_path),
    )

    await _assertions_for_profile_without_frozen_module(command, file_path, profiler)


@bsmr_test(skip_for_os=["windows"])
async def test_profile_patterns(bsmr: Bsmr, tmp_path: Path) -> None:
    with TemporaryDirectory() as tmpdir:
        tmp_path = Path(tmpdir)

        await bsmr.build(
            "//simple/...",
            "--profile-patterns=.*",
            "--profile-patterns-mode=statement",
            f"--profile-patterns-output={tmp_path}",
        )

        # Use paths relative to tmpdir instead of just filenames
        files_with_sizes = []
        for root, _, files in os.walk(tmp_path):
            for fname in files:
                fpath = Path(root) / fname
                try:
                    size = fpath.stat().st_size
                except FileNotFoundError:
                    continue
                rel_path = fpath.relative_to(tmp_path)

                # Drop the first path component from rel_path before storing (it's nondeterministic of the format <timestamp>-<builduuid>)
                assert len(rel_path.parts) > 1
                rel_path = Path(*rel_path.parts[1:])

                files_with_sizes.append((str(rel_path), size))

        # Sort by filename
        files_with_sizes.sort(key=lambda x: x[0])

        # Format as "file: True/False" where bool indicates non-zero size
        output_lines = [f"{fname}: {size > 0}" for fname, size in files_with_sizes]
        golden(output="\n".join(output_lines), rel_path="profile_patterns.golden")


@bsmr_test(skip_for_os=["windows"])
async def test_profile_patterns_flame(bsmr: Bsmr, tmp_path: Path) -> None:
    with TemporaryDirectory() as tmpdir:
        tmp_path = Path(tmpdir)

        await bsmr.build(
            "//simple/...",
            "--profile-patterns=.*",
            "--profile-patterns-mode=time-flame",
            f"--profile-patterns-output={tmp_path}",
        )

        # Use paths relative to tmpdir instead of just filenames
        files_with_sizes = []
        for root, _, files in os.walk(tmp_path):
            for fname in files:
                fpath = Path(root) / fname
                try:
                    size = fpath.stat().st_size
                except FileNotFoundError:
                    continue
                rel_path = fpath.relative_to(tmp_path)

                # Drop the first path component from rel_path before storing (it's nondeterministic of the format <timestamp>-<builduuid>)
                assert len(rel_path.parts) > 1
                rel_path = Path(*rel_path.parts[1:])

                files_with_sizes.append((str(rel_path), size))

        # Sort by filename
        files_with_sizes.sort(key=lambda x: x[0])

        # Format as "file: True/False" where bool indicates non-zero size
        output_lines = [f"{fname}: {size > 0}" for fname, size in files_with_sizes]
        golden(output="\n".join(output_lines), rel_path="profile_patterns_flame.golden")


async def _assertions_for_profile_without_frozen_module(
    command: Process[BsmrResult, BsmrException],
    file_path: Path,
    profiler: str,
) -> None:
    if profiler.endswith("-retained"):
        await expect_failure(
            command,
            stderr_regex="Retained memory profiling is available only for analysis profile",
        )
    else:
        await assert_flame_outputs(command, file_path, profiler)


async def assert_flame_outputs(
    command: Process[BsmrResult, BsmrException],
    file_path: Path,
    profiler: str,
) -> None:
    await command

    assert os.path.exists(file_path)
    if "flame" in profiler:
        assert os.path.exists(file_path / "flame.src")
        assert os.path.exists(file_path / "flame.svg")
    else:
        assert os.path.exists(file_path / "profile.csv")

    assert os.path.exists(file_path / "targets.txt")
