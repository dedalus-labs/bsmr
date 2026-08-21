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
import os
import subprocess

from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


@bsmr_test(inplace=True)
async def test_rust_binary() -> None:
    rust_project_bin = os.environ["RUST_PROJECT_BIN"]

    env = os.environ.copy()
    env["BSMR_HARD_ERROR"] = "false"

    result = subprocess.run(
        [
            rust_project_bin,
            "develop",
            "--stdout",
            "--pretty",
            "root//tests/targets/rules/rust/hello_world:welcome",
        ],
        stdout=subprocess.PIPE,
        env=env,
    )

    json_generated = json.loads(result.stdout)

    assert "sysroot" in json_generated.keys()
    assert "sysroot_src" in json_generated.keys()
    assert "crates" in json_generated.keys()
