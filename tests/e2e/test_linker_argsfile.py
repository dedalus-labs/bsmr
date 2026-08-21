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

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test, get_mode_from_platform


@bsmr_test(inplace=True)
async def test_linker_argsfile_valid(bsmr: Bsmr) -> None:
    args = [
        "root//tests/targets/rules/cxx/hello_world:welcome[linker.argsfile]",
        "--show-full-output",
        get_mode_from_platform(),
    ]
    result = await bsmr.build(*args)
    output_dict = result.get_target_to_build_output()
    assert len(output_dict) == 1
    output_path = next(iter(output_dict.values()))
    # Ensure that the argsfile exists and is not empty.
    assert os.path.exists(output_path)
    assert os.path.getsize(output_path) > 0
