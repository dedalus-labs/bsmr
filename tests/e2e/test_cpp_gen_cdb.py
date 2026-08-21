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
import platform
from pathlib import Path

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


# TODO(marwhal): Fix and enable on Windows
@bsmr_test(inplace=True, skip_for_os=["windows"])
async def test_no_quotes(bsmr: Bsmr) -> None:
    result = await bsmr.bxl(
        "upstream//tools/build/bsmr/bxl/cpp_lsp/cpp_gen_cdb.bxl:cpp_gen_cdb",
        "--",
        "--filename",
        str(
            bsmr.cwd.parent
            / "fbcode/bsmr/tests/targets/cpp_gen_cdb/basic/src/main.cpp"
        ),
        "--os",
        platform.system().lower(),
        "--exec_mode",
        "local",
    )
    outputs = json.loads(result.stdout)
    compdb_path = Path(outputs["compilationDatabasePath"]) / ".." / "compdb.json"

    with open(compdb_path) as f:
        commands = json.load(f)

    # check that the define is present without any shell quotes
    arguments = commands[0]["arguments"]
    assert arguments.index("-DM_FOO_BAR=1")


# TODO(marwhal): Add this back one at least one test in this file passes on Windows
@bsmr_test(inplace=True)
async def test_noop(bsmr: Bsmr) -> None:
    return
