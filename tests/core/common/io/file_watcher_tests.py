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


import subprocess
from enum import Enum

from bsmr.tests.core.common.io.file_watcher import FileWatcherEvent
from bsmr.tests.core.common.io.utils import get_files
from bsmr.tests.e2e_util.api.bsmr import Bsmr


class FileSystemType(Enum):
    NATIVE = 0
    EDEN_FS = 1


async def setup_file_watcher_test(bsmr: Bsmr) -> None:
    # Fails on eden because the repo exists, that's ok
    subprocess.run(["sl", "init"], cwd=bsmr.cwd)
    subprocess.run(["sl", "commit", "--addremove", "-m", "temp"], cwd=bsmr.cwd)
    subprocess.run(["sl", "bookmark", "main"], cwd=bsmr.cwd, check=True)

    sl_status = subprocess.check_output(["sl", "status"], cwd=bsmr.cwd)
    assert sl_status == b"", (
        f"Expected clean working directory, but `sl status` returned:\n{sl_status.decode(errors='replace')}"
    )
    assert (await get_files(bsmr)) == ["files/abc", "files/d/empty"]


def verify_results(
    results: list[FileWatcherEvent],
    required: list[FileWatcherEvent],
) -> None:
    for req in required:
        if req not in results:
            print(f"results={results}")
            print(f"required={required}")
            assert req in results, "required not in results"


async def run_aba_test(bsmr: Bsmr) -> None:
    await setup_file_watcher_test(bsmr)

    subprocess.run(["sl", "mv", "files/abc", "files/d/"], cwd=bsmr.cwd, check=True)
    assert (await get_files(bsmr)) == ["files/d/abc", "files/d/empty"]

    subprocess.run(["sl", "shelve"], cwd=bsmr.cwd, check=True)
    assert (await get_files(bsmr)) == ["files/abc", "files/d/empty"]
