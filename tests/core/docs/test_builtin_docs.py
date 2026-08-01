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

from bsmr.tests.e2e_util.api.buck import Buck
from bsmr.tests.e2e_util.buck_workspace import buck_test
from bsmr.tests.e2e_util.helper.golden import golden_dir


@buck_test()
async def test_builtin_docs_golden(buck: Buck) -> None:
    output = buck.cwd.parent / "output"
    await buck.docs("starlark-builtins", "--output-dir", str(output))

    outputs: dict[str, str] = {}
    for file in output.glob("**/*.md"):
        lines = file.read_text(encoding="utf-8").splitlines()
        lines = filter(lambda x: x.startswith("# ") or x.startswith("## "), lines)
        s = "\n".join(lines)

        rel_path = file.relative_to(output)
        outputs[str(rel_path)] = s

    golden_dir(output=outputs, rel_path="bsmr-golden-docs")
