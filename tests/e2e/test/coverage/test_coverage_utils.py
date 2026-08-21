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
    folder_filter: List[str],
    file_filter: List[str],
    mode: Optional[str] = None,
    extra_tpx_args: Optional[List[str]] = None,
    extra_bsmr_args: Optional[List[str]] = None,
) -> List[str]:
    coverage_file = tmp_path / "coverage.txt"
    folder_filter_str = ":".join(folder_filter)
    file_filter_str = ":".join(file_filter)
    bsmr_args = []
    if mode is not None:
        bsmr_args.append(mode)
    bsmr_args.extend(
        [
            "--config",
            "code_coverage.enable=filtered",
            "--config",
            f"code_coverage.folder_path_filter={folder_filter_str}",
            "--config",
            f"code_coverage.file_path_filter={file_filter_str}",
        ]
        + (extra_bsmr_args or [])
    )
    bsmr_args.append(target)
    bsmr_args.extend(
        [
            "--",
            "--collect-coverage",
            f"--coverage-output={coverage_file}",
        ]
        + (extra_tpx_args or [])
    )
    await bsmr.test(*bsmr_args)
    paths = []
    with open(coverage_file) as results:
        for line in results:
            paths.append(json.loads(line)["filepath"])

    return list(set(paths))
