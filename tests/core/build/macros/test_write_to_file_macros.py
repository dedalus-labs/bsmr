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
import re

from bsmr.tests.e2e_util.api.bsmr import Bsmr
from bsmr.tests.e2e_util.bsmr_workspace import bsmr_test


def _normalize_path(p: str) -> str:
    p = p.replace("\\", "/")
    p = re.sub("/([a-f0-9]{16})/", "/<HASH>/", p)
    p = re.sub("/([a-f0-9]{40})/", "/<HASH>/", p)
    return p


def _find_file(dir, name: str) -> str:
    files = []
    for root, _, filenames in os.walk(dir):
        for filename in filenames:
            if filename == name:
                files.append(os.path.join(root, filename))
    [f] = files
    return f


@bsmr_test(setup_eden=True)
async def test_xxx(bsmr: Bsmr) -> None:
    result = await bsmr.build("//:test_rule")
    out = result.get_build_report().output_for_target("root//:test_rule")

    # Out contents is:
    # ```
    # @bsmr-out/default/art/root/6dd044292ff31ae1/__test_rule__/__macros/1e0e364a22c69340e6f02604520fdeb7674264c0/0.macro
    # @../__macros/1e0e364a22c69340e6f02604520fdeb7674264c0/1.macro
    # @../__macros/1e0e364a22c69340e6f02604520fdeb7674264c0/2.macro
    # @../__macros/1e0e364a22c69340e6f02604520fdeb7674264c0/3.macro
    # ```

    with open(out) as f:
        [a, b, c, d] = [line.strip() for line in f.readlines()]

    a = a.replace("\\", "/")
    b = b.replace("\\", "/")
    c = c.replace("\\", "/")
    d = d.replace("\\", "/")

    assert (
        "@bsmr-out/default/art/root/<HASH>/__test_rule__/__macros/<HASH>/0.macro"
        == _normalize_path(a)
    )
    assert "@../__macros/<HASH>/1.macro" == _normalize_path(b)
    assert "@../__macros/<HASH>/2.macro" == _normalize_path(c)
    assert "@../__macros/<HASH>/3.macro" == _normalize_path(d)

    output = bsmr.cwd / "bsmr-out"
    a_x = _find_file(output, "0.macro")
    with open(a_x) as f:
        a_contents = _normalize_path(f.read())
        assert "bsmr-out/default/art/root/<HASH>/__write_file__/write_file.txt" == a_contents

    # TODO(nga): contents of `{1,2,3}.macro` should be identical.

    b_x = _find_file(output, "1.macro")
    with open(b_x) as f:
        b_contents = _normalize_path(f.read())
        assert "bsmr-out/default/art/root/<HASH>/__write_file__/write_file.txt" == b_contents

    c_x = _find_file(output, "2.macro")
    with open(c_x) as f:
        c_contents = _normalize_path(f.read())
        assert "../../__write_file__/write_file.txt" == c_contents

    d_x = _find_file(output, "3.macro")
    with open(d_x) as f:
        d_contents = _normalize_path(f.read())
        assert "../../__write_file__/write_file.txt" == d_contents
