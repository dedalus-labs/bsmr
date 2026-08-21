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
from pathlib import Path
from typing import List, Optional

from bsmr.tests.e2e_util.api.bsmr import Bsmr


async def collect_coverage_for(
    bsmr: Bsmr,
    tmp_path: Path,
    target: str,
    filter: List[str],
    mode: Optional[str] = None,
) -> List[str]:
    coverage_file = tmp_path / "coverage.txt"
    filter_str = " ".join(filter)
    bsmr_args = []
    if mode is not None:
        bsmr_args.append(mode)
    bsmr_args.extend(
        [
            "--config",
            "fbcode.coverage_selective=true",
            "--config",
            f"fbcode.cxx_coverage_only={filter_str}",
            target,
            "--",
            "--collect-coverage",
            f"--coverage-output={coverage_file}",
        ]
    )
    await bsmr.test(*bsmr_args)
    paths = []
    with open(coverage_file) as results:
        for line in results:
            paths.append(json.loads(line)["filepath"])

    return list(set(paths))
