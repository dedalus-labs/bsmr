# ===----------------------------------------------------------------------===
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

def _strip_third_party_rust_version(target: str) -> str:
    # When upgrading libraries we either suffix them as `-old` or with a version, e.g. `-1-08`
    # Strip those so we grab the right one in open source.
    if target.endswith(":md-5"):  # md-5 is the one exception
        return target
    xs = target.split("-")
    for i in reversed(range(len(xs))):
        s = xs[i]
        if s == "old" or s.isdigit():
            xs.pop(i)
        else:
            break
    return "-".join(xs)

# Cell the BUILD.bsmr file being processed belongs to
ACTIVE_CELL = native.get_cell_name()

# The cell containing this build-support layer.
BUILD_CELL = read_config("oss", "build_cell", "bsmr_build")

# Some third-party manifests use the generic `third-party` cell. Resolve those
# dependencies to Bessemer's build-support cell.
THIRD_PARTY_REWRITE_RULES = {
    "third-party": struct(
        dirs = [
            ("", "third-party"),
        ],
        dynamic = [
            ("rust", lambda path: "third-party/" + _strip_third_party_rust_version(path)),
        ],
    ),
}

"""
Resolve generic third-party labels to Bessemer's build-support cell.
"""

def translate_target(target: str) -> str:
    if "//" not in target:
        # This is a local target, aka ":foo". Don't touch
        return target

    (cell, path) = target.split("//", 1)

    resolved_cell = ACTIVE_CELL if cell == "" else cell
    rules = THIRD_PARTY_REWRITE_RULES.get(resolved_cell)

    if rules == None:
        # No implicit rewrite rules
        return target

    for match_root_dir, fn in getattr(rules, "dynamic", []):
        if _path_rooted_in_dir(path, match_root_dir):
            return BUILD_CELL + "//" + fn(path)

    for match_root_dir, replace_root_dir in getattr(rules, "dirs", []):
        if _path_rooted_in_dir(path, match_root_dir):
            return BUILD_CELL + "//" + _swap_root_dir_for_path(path, match_root_dir, replace_root_dir)

    return target

def _path_rooted_in_dir(path: str, d: str) -> bool:
    return d == "" or path == d or path.startswith(d + "/") or path.startswith(d + ":")

def _strip_root_dir_from_path(path: str, d: str) -> str:
    return path.removeprefix(d).removeprefix("/")

def _swap_root_dir_for_path(path: str, root_dir: str, new_root_dir) -> str:
    suffix = _strip_root_dir_from_path(path, root_dir)
    if not suffix.startswith(":"):
        suffix = "/" + suffix
    replace_path = new_root_dir.removesuffix("/") + suffix
    return replace_path.removeprefix("/")
