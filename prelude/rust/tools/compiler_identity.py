#!/usr/bin/env python3
# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

"""Identify a supported system compiler before cache lookup, then dispatch to it.

Overlapping commands share one observation. The compiler and its runtime files
must remain unchanged until those commands finish. Concurrent upgrades are unsupported.
"""

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import TypedDict, cast


class Identity(TypedDict):
    """Content identity and executable selected for one command."""

    compiler: str
    version: str
    files: list[tuple[str, str, str]]


class UnsupportedToolchainError(RuntimeError):
    """The system compiler does not have the supported self-contained layout."""


def system_library(path: Path, resolved: Path) -> bool:
    """Allow named OS runtime libraries outside the compiler's hashed file set."""
    names = {
        "libc.so.6",
        "libm.so.6",
        "libdl.so.2",
        "librt.so.1",
        "libpthread.so.0",
        "libgcc_s.so.1",
        "libstdc++.so.6",
        "libz.so.1",
        "libutil.so.1",
        "ld-linux-x86-64.so.2",
        "ld-linux-aarch64.so.1",
        "ld-linux-riscv64-lp64d.so.1",
    }
    ret = path.name in names and any(
        resolved.is_relative_to(root) for root in ["/lib", "/lib64", "/usr/lib", "/usr/lib64"]
    )
    return ret


@dataclass(frozen=True, slots=True, kw_only=True)
class Io:
    """Read the installed compiler and its bounded runtime closure."""

    sysroot: Path

    def file(self, path: Path) -> tuple[str, str, str]:
        """Hash one file and bind both its name and its in-sysroot resolution."""
        resolved = path.resolve(strict=True)
        if not resolved.is_relative_to(self.sysroot):
            raise UnsupportedToolchainError(f"runtime file escapes sysroot: {path}")
        if not resolved.is_file():
            raise UnsupportedToolchainError(f"runtime entry is not a regular file: {path}")
        with path.open("rb") as file:
            sha = hashlib.file_digest(file, "sha256")
        ret = (
            str(path.relative_to(self.sysroot)),
            str(resolved.relative_to(self.sysroot)),
            sha.hexdigest(),
        )
        return ret

    def tree(self, root: Path) -> list[tuple[str, str, str]]:
        """Hash runtime tree membership without following directory symlinks."""
        if root.is_symlink():
            raise UnsupportedToolchainError(f"runtime directory is a symlink: {root}")
        ret = []
        for path in sorted(root.iterdir()):
            if path.is_dir():
                ret.extend(self.tree(root=path))
            else:
                ret.append(self.file(path=path))
        return ret

    def loaded_libraries(self, compiler: Path, files: list[tuple[str, str, str]]) -> None:
        """Ask the native loader to resolve dependencies and reject untracked compiler libraries."""
        res = subprocess.run(
            [compiler, "-vV"],
            check=True,
            capture_output=True,
            text=True,
            env={**os.environ, "LC_ALL": "C", "LD_TRACE_LOADED_OBJECTS": "1"},
        )
        trace = res.stdout
        if not trace.strip():
            raise UnsupportedToolchainError("loader did not report its dependency paths")
        tracked = {self.sysroot / resolved for _, resolved, _ in files}
        for line in trace.splitlines():
            if re.fullmatch(r"linux-vdso(?:64)?\.so\.1 \(0x[0-9a-fA-F]+\)", line.strip()):
                continue
            match = re.fullmatch(r"(?:\S+ => )?(/.*) \(0x[0-9a-fA-F]+\)", line.strip())
            if match is None:
                raise UnsupportedToolchainError(f"unresolved or unrecognized loader record: {line}")
            path = Path(match[1])
            resolved = path.resolve(strict=True)
            if system_library(path=path, resolved=resolved):
                continue
            if resolved not in tracked:
                raise UnsupportedToolchainError(
                    f"loaded library is not a hashed runtime file: {path}"
                )

    def record(self, target: str, output: Path) -> None:
        """Record the Linux rustup compiler and host/target runtime trees."""
        compiler = self.sysroot / "bin" / "rustc"
        files = [self.file(path=compiler)]
        libraries = self.sysroot / "lib"
        if not list(libraries.glob("librustc_driver-*.so")):
            raise UnsupportedToolchainError(f"no compiler driver directly in {libraries}")
        for path in sorted(libraries.iterdir()):
            if path.name != "rustlib":
                files.append(self.file(path=path))
        res = subprocess.run([compiler, "-vV"], check=True, capture_output=True, text=True)
        host = next(
            (
                line.removeprefix("host: ")
                for line in res.stdout.splitlines()
                if line.startswith("host: ")
            ),
            None,
        )
        if host is None:
            raise UnsupportedToolchainError("rustc -vV did not report its host")
        rustlib = libraries / "rustlib"
        if rustlib.is_symlink():
            raise UnsupportedToolchainError(f"runtime directory is a symlink: {rustlib}")
        for triple in sorted({host, target}):
            runtime = rustlib / triple
            if not (runtime / "lib").is_dir():
                raise UnsupportedToolchainError(
                    f"{self.sysroot} has no standard library for {triple}"
                )
            files.extend(self.tree(root=runtime))
        self.loaded_libraries(compiler=compiler, files=files)
        identity: Identity = {"compiler": str(compiler), "version": res.stdout, "files": files}
        output.write_text(json.dumps(identity, indent=1) + "\n")

    @staticmethod
    def discover(target: str, output: Path) -> None:
        """Resolve PATH once and reject unsupported platforms and loader overrides."""
        if sys.platform != "linux":
            raise UnsupportedToolchainError(f"unsupported platform: {sys.platform}")
        for name in sorted(os.environ):
            if name.startswith(("LD_", "DYLD_")):
                raise UnsupportedToolchainError(f"loader override is set: {name}")
        proxy = shutil.which("rustc")
        if proxy is None:
            raise UnsupportedToolchainError("rustc is not on PATH")
        res = subprocess.run(
            [proxy, "--print", "sysroot"], check=True, capture_output=True, text=True
        )
        io = Io(sysroot=Path(res.stdout.strip()).resolve(strict=True))
        io.record(target=target, output=output)

    @staticmethod
    def run(identity: Path, arguments: list[str]) -> None:
        """Execute the compiler selected by discovery without revalidating its files."""
        pinned = cast(Identity, json.loads(identity.read_text()))
        res = subprocess.run([pinned["compiler"], *arguments])
        sys.exit(res.returncode)


def main() -> None:
    """Dispatch discovery or execution and report unsupported layouts explicitly."""
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    recording = commands.add_parser("record")
    recording.add_argument("--target", required=True)
    recording.add_argument("output", type=Path)
    running = commands.add_parser("run")
    running.add_argument("identity", type=Path)
    running.add_argument("arguments", nargs=argparse.REMAINDER)
    args = parser.parse_args()
    try:
        if args.command == "run":
            Io.run(identity=args.identity, arguments=args.arguments)
            return
        Io.discover(target=args.target, output=args.output)
    except subprocess.CalledProcessError as error:
        raise SystemExit(f"unsupported system Rust toolchain: {error.stderr.strip()}") from error
    except (UnsupportedToolchainError, OSError) as error:
        raise SystemExit(f"unsupported system Rust toolchain: {error}") from error


if __name__ == "__main__":
    main()
