#!/usr/bin/env python3
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


from asyncio import subprocess
from pathlib import Path
from typing import Callable, Dict, Iterable, TypeVar


class Result:
    """
    Represents a process that has finished running and succeeded.
    If the bsmr process failed, it should raise Exception
    """

    def __init__(
        self,
        process: subprocess.Process,
        stdout: str,
        stderr: str,
    ) -> None:
        self.process = process
        self.stdout = stdout
        self.stderr = stderr
        self.check_returncode()

    def check_returncode(self) -> None:
        assert self.process.returncode == 0


R = TypeVar("R", bound=Result, covariant=True)
E = TypeVar("E", bound=Exception)
ResultType = Callable[[subprocess.Process, str, str], R]
ExceptionType = Callable[
    [Iterable[str], Path, Dict[str, str], subprocess.Process, str, str], E
]
