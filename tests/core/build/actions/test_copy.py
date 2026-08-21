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


import pytest
from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


@pytest.mark.parametrize("executable_bit_override", [None, True, False])
@pytest.mark.parametrize("write_executable_bit", [None, True, False])
@pytest.mark.parametrize(
    "src,is_executable",
    [
        (None, None),
        ("files/is_executable.sh", True),
        ("files/not_executable.sh", False),
        ("files/executable_scripts", True),
        ("files/not_executable_scripts", False),
    ],
)
@bsmr_test(skip_for_os=["windows"])  # Exec bit and all
async def test_exec_bit_of_copied_file(
    bsmr: Bsmr,
    executable_bit_override: bool | None,
    write_executable_bit: bool | None,
    src: str | None,
    is_executable: bool | None,
) -> None:
    if src is None and write_executable_bit is None:
        return
    if src is not None and write_executable_bit is not None:
        return

    if executable_bit_override is not None:
        is_executable = executable_bit_override
    elif write_executable_bit is not None:
        is_executable = write_executable_bit

    assert is_executable is not None

    name = "perms_{}_{}_{}".format(executable_bit_override, write_executable_bit, src)

    res = await bsmr.build_without_report(
        f":{name}", "--out=-", "--local-only", "--no-remote-cache"
    )

    expected_val = "x" if is_executable else "-"

    for line in res.stdout.strip().split():
        line = line.strip()
        assert line[3] == expected_val
        assert line[6] == expected_val
        assert line[9] == expected_val


@bsmr_test()  # Make sure there's at least one test defined
async def test_dummy(bsmr: Bsmr) -> None:
    pass
