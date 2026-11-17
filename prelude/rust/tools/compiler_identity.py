#!/usr/bin/env python3
# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Pins the system Rust compiler that a build resolves from PATH.

"""Record the system Rust compiler, or run exactly the recorded compiler.

`record` resolves `rustc` once, then writes the real compiler path, its `-vV`
report and a digest of each sysroot file a compile loads. Compiles take the
record as an input, so it is part of every action key.

`run` executes the recorded compiler. It fails when a recorded file changed
after the record was written, before or during the compile, so a result never
enters the cache under the key of another compiler.
"""

import argparse
import hashlib
import json
import shutil
import subprocess
import sys
from pathlib import Path


def stamp(path: Path) -> list[int]:
    """Return the size and modification time that a replaced file changes."""
    status = path.stat()
    ret = [status.st_size, status.st_mtime_ns]
    return ret


def digest(path: Path) -> str:
    """Return the SHA-256 digest of one file's contents."""
    sha = hashlib.sha256()
    with path.open("rb") as file:
        for chunk in iter(lambda: file.read(1 << 20), b""):
            sha.update(chunk)
    ret = sha.hexdigest()
    return ret


def record(target: str, output: Path) -> None:
    """Write the identity of the compiler and sysroot that rustc resolves to."""
    proxy = shutil.which("rustc")
    if proxy is None:
        raise SystemExit("rustc is not on PATH")
    res = subprocess.run([proxy, "--print", "sysroot"], check=True, capture_output=True, text=True)
    sysroot = Path(res.stdout.strip())
    compiler = sysroot / "bin" / Path(proxy).name
    libraries = sysroot / "lib"
    standard = libraries / "rustlib" / target / "lib"
    if not standard.is_dir():
        raise SystemExit(f"{sysroot} has no standard library for {target}")
    res = subprocess.run([compiler, "-vV"], check=True, capture_output=True, text=True)
    paths = [
        compiler,
        # A distribution sysroot may be /usr, so take only the compiler's own
        # libraries from lib/. The -vV report names the LLVM version as well.
        *sorted([*libraries.glob("librustc_driver-*"), *libraries.glob("libLLVM*")]),
        *sorted(path for path in standard.parent.rglob("*") if path.is_file()),
    ]
    files = [[str(path.relative_to(sysroot)), digest(path), *stamp(path)] for path in paths]
    identity = {"compiler": str(compiler), "version": res.stdout, "files": files}
    output.write_text(json.dumps(identity, indent=1) + "\n")


def check(sysroot: Path, files: list[tuple[str, str, int, int]]) -> None:
    """Fail unless every recorded file still has its recorded size and time."""
    for name, _, size, mtime in files:
        path = sysroot / name
        try:
            unchanged = stamp(path) == [size, mtime]
        except FileNotFoundError:
            unchanged = False
        if not unchanged:
            raise SystemExit(f"{path} changed after this build recorded it; run `bsmr kill`")


def run(identity: Path, arguments: list[str]) -> None:
    """Run the recorded compiler, checking its files before and after."""
    pinned = json.loads(identity.read_text())
    compiler = Path(pinned["compiler"])
    sysroot = compiler.parent.parent
    check(sysroot=sysroot, files=pinned["files"])
    res = subprocess.run([compiler, *arguments])
    check(sysroot=sysroot, files=pinned["files"])
    sys.exit(res.returncode)


def main() -> None:
    """Dispatch to record or run."""
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    recording = commands.add_parser("record")
    recording.add_argument("--target", required=True)
    recording.add_argument("output", type=Path)
    running = commands.add_parser("run")
    running.add_argument("identity", type=Path)
    running.add_argument("arguments", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    if args.command == "record":
        record(target=args.target, output=args.output)
        return
    run(identity=args.identity, arguments=args.arguments)


if __name__ == "__main__":
    main()
