# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

"""Check source snapshot links after native artifact materialization."""

import argparse
import json
import os
import tempfile
from pathlib import Path

from macros import run

RULE = '''
def _impl(ctx):
    package = ctx.actions.copied_dir("package", ctx.attrs.files, SYMLINKS)
    checkout = ctx.actions.copied_dir("checkout", {"pkg": package}, SYMLINKS)
    return [DefaultInfo(default_output = checkout)]

snapshot = rule(impl = _impl, attrs = {"files": attrs.dict(attrs.string(), attrs.source())})
'''


def qualify(binary: str, root: Path, *, preserve: bool) -> None:
    """Require both content and relative-link text to survive two copy stages."""
    (root / "data").mkdir()
    (root / "data/file").write_text("snapshot source\n")
    (root / "data/link").symlink_to("./file")
    option = 'symlinks = "preserve"' if preserve else ""
    (root / "copy.bzl").write_text(RULE.replace("SYMLINKS", option))
    (root / "BUILD.bsmr").write_text(
        'load(":copy.bzl", "snapshot")\n'
        'snapshot(name = "tree", files = {"file": "data/file", "link": "data/link"})\n'
    )
    initialized = run(root, binary, "init")
    assert initialized.returncode == 0, initialized.stderr
    try:
        built = run(root, binary, "build", "//:tree", "--show-full-json-output")
        assert built.returncode == 0, built.stderr
        outputs = json.loads(built.stdout)
        tree = root / next(iter(outputs.values())) / "pkg"
        assert (tree / "file").read_text() == "snapshot source\n"
        actual = os.readlink(tree / "link")
        assert actual == "./file", f"snapshot rewrote the link target: {actual!r}"
        assert (tree / "link").read_text() == "snapshot source\n"
        print("ok: source symlink survives nested native copies")
    finally:
        stopped = run(root, binary, "kill")
        assert stopped.returncode == 0, stopped.stderr


def main() -> None:
    """Select a compiler and retain the old copy mode for the red regression run."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("--old-copy", action="store_true")
    arguments = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="bsmr-copy-") as directory:
        qualify(str(arguments.binary.resolve()), Path(directory), preserve=not arguments.old_copy)


if __name__ == "__main__":
    main()
