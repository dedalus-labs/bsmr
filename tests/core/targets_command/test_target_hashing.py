# ===----------------------------------------------------------------------===
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


import os
from pathlib import Path

from bsmr.tests.e2e_util.api.buck import Buck
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.buck_workspace import buck_test


@buck_test()
async def test_target_hashing_accepts_backreferencing_relative_paths(
    buck: Buck,
    tmp_path: Path,
) -> None:
    await buck.targets(
        ":bin",
        "--show-target-hash",
        "--target-hash-file-mode=paths_only",
        "--target-hash-modified-paths=../.bsmr",
        rel_cwd=Path("bin"),
    )

    # Paths outside of the project still fail
    await expect_failure(
        buck.targets(
            "bin:bin",
            "--show-target-hash",
            "--target-hash-file-mode=paths_only",
            "--target-hash-modified-paths=../.bsmr",
        ),
        stderr_regex="relativize path.*against project root",
    )

    if os.name != "nt":
        # Absolute path non-normalized paths should work
        (tmp_path / "symlink").symlink_to(buck.cwd)

        await buck.targets(
            ":bin",
            "--show-target-hash",
            "--target-hash-file-mode=paths_only",
            f"--target-hash-modified-paths={tmp_path}/symlink/.bsmr",
            rel_cwd=Path("bin"),
        )
