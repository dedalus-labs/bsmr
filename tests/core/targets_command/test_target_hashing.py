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


import os
from pathlib import Path

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


@bsmr_test()
async def test_target_hashing_accepts_backreferencing_relative_paths(
    bsmr: Bsmr,
    tmp_path: Path,
) -> None:
    await bsmr.targets(
        ":bin",
        "--show-target-hash",
        "--target-hash-file-mode=paths_only",
        "--target-hash-modified-paths=../.bsmr",
        rel_cwd=Path("bin"),
    )

    # Paths outside of the project still fail
    await expect_failure(
        bsmr.targets(
            "bin:bin",
            "--show-target-hash",
            "--target-hash-file-mode=paths_only",
            "--target-hash-modified-paths=../.bsmr",
        ),
        stderr_regex="relativize path.*against project root",
    )

    if os.name != "nt":
        # Absolute path non-normalized paths should work
        (tmp_path / "symlink").symlink_to(bsmr.cwd)

        await bsmr.targets(
            ":bin",
            "--show-target-hash",
            "--target-hash-file-mode=paths_only",
            f"--target-hash-modified-paths={tmp_path}/symlink/.bsmr",
            rel_cwd=Path("bin"),
        )
