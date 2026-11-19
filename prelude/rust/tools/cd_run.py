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

# Changes directory and then runs a command

import argparse
import os
import subprocess
import sys


def main() -> None:
    """Resolve declared environment paths before changing the command directory."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--path-env", action="append", default=[])
    parser.add_argument("directory")
    parser.add_argument("command", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    for key in args.path_env:
        os.environ[key] = os.path.abspath(os.environ[key])
    res = subprocess.run(args.command, cwd=args.directory)
    sys.exit(res.returncode)


if __name__ == "__main__":
    main()
