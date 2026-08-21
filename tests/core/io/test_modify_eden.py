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


from pathlib import Path

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


@bsmr_test(setup_eden=True)
async def test_modify_src_eden(bsmr: Bsmr) -> None:
    path = bsmr.cwd / "src.txt"

    path.write_text("HELLO\n")
    result = await bsmr.build("root//:copy_file")
    output = result.get_build_report().output_for_target("root//:copy_file")
    assert Path(output).read_text() == "HELLO\n"

    path.write_text("GOODBYE\n")
    result = await bsmr.build("root//:copy_file")
    output = result.get_build_report().output_for_target("root//:copy_file")
    assert Path(output).read_text() == "GOODBYE\n"
