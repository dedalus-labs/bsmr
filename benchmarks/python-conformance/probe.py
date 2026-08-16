# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Records deterministic import success and failure behavior for one environment.

"""Probe imports without depending on third-party Python packages."""

from __future__ import annotations

import argparse
import importlib
import json
import sys


def _arguments() -> argparse.Namespace:
    """Parse exact import roots and module names."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--import", action="append", default=[], dest="imports")
    parser.add_argument("--root", action="append", default=[])
    return parser.parse_args()


def main() -> None:
    """Import every requested module and serialize stable outcomes."""
    args = _arguments()
    sys.path[:0] = args.root
    observations = []
    for name in args.imports:
        try:
            importlib.import_module(name)
            observations.append({"name": name, "ok": True})
        except Exception as error:  # noqa: BLE001 - exception identity is the conformance output.
            observations.append(
                {
                    "error": str(error),
                    "name": name,
                    "ok": False,
                    "type": type(error).__name__,
                }
            )
    print(json.dumps(observations, separators=(",", ":"), sort_keys=True))


if __name__ == "__main__":
    main()
