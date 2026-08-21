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

import sys
from os.path import exists
from pathlib import Path

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.asserts import expect_failure
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


# Currently installer grpc doesn't compile on Mac
def linux_only() -> bool:
    return sys.platform == "linux"


if linux_only():

    @bsmr_test(inplace=True)
    async def test_install_modifiers(bsmr: Bsmr, tmp_path: Path) -> None:
        tmp_dir = tmp_path / "no_modifiers"
        tmp_dir.mkdir()
        args = ["--dst", f"{tmp_dir}/"]

        await bsmr.install(
            "root//tests/targets/rules/install:installer_modifiers_test",
            "--",
            *args,
        )

        assert exists(f"{tmp_dir}/default")

        tmp_dir = tmp_path / "modifiers"
        tmp_dir.mkdir()
        args = ["--dst", f"{tmp_dir}/"]

        await bsmr.install(
            "root//tests/targets/rules/install:installer_modifiers_test?asan",
            "--",
            *args,
        )

        assert exists(f"{tmp_dir}/asan")


@bsmr_test(inplace=True)
async def test_install_fails_with_global_modifiers(bsmr: Bsmr, tmp_path: Path) -> None:
    tmp_dir = tmp_path / "install_test"
    tmp_dir.mkdir()
    args = ["--dst", f"{tmp_dir}/"]
    await expect_failure(
        bsmr.install(
            "--modifier",
            "asan",
            "root//tests/targets/rules/install:installer_modifiers_test?asan",
            "--",
            *args,
        ),
        stderr_regex=r"Cannot specify modifiers with \?modifier syntax when global CLI modifiers are set with --modifier flag",
    )
