# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

"""Reject stale compiler identities before in-memory and disk-cache reuse."""

import argparse
import os
import shutil
import tempfile
from pathlib import Path

from system import Io


def copied_toolchain(io: Io, channel: str) -> Path:
    """Copy real installed compiler bytes into a private, mutable sysroot."""
    root = Path(io.run("rustup", "run", channel, "rustc", "--print", "sysroot").stdout.strip())
    destination = io.project.parent / "toolchain"
    (destination / "bin").mkdir(parents=True)
    shutil.copy2(root / "bin" / "rustc", destination / "bin" / "rustc")
    shutil.copytree(root / "lib", destination / "lib", symlinks=True)
    return destination


def replace_trailer(path: Path, value: bytes) -> None:
    """Replace one inert ELF trailer byte without changing size or mtime."""
    status = path.stat()
    with path.open("r+b") as file:
        file.seek(-1, os.SEEK_END)
        file.write(value)
    os.utime(path, ns=(status.st_atime_ns, status.st_mtime_ns))
    assert path.stat().st_size == status.st_size
    assert path.stat().st_mtime_ns == status.st_mtime_ns


def library_build(io: Io) -> dict[str, str]:
    """Build the verified library without linking an executable."""
    result = io.run(io.binary, "build", ":leaf", "--console", "simple")
    return io.executors(result)


def replaces_compiler_before_memory_reuse(io: Io, compiler: Path) -> None:
    """An unchanged target must recompile when compiler content changes."""
    first = library_build(io)
    assert first == {"root//:leaf": "Local"}, first
    assert library_build(io) == {}
    version = io.run(str(compiler), "-vV").stdout
    replace_trailer(compiler, b"1")
    assert io.run(str(compiler), "-vV").stdout == version
    changed = library_build(io)
    assert changed == {"root//:leaf": "Local"}, changed


def replaces_library_before_disk_hit(io: Io, driver: Path) -> None:
    """A previous source variant must miss after a compiler library changes."""
    source = io.project / "leaf.rs"
    original = source.read_bytes()
    source.write_bytes(original + b"\n")
    changed = library_build(io)
    assert changed == {"root//:leaf": "Local"}, changed
    source.write_bytes(original)
    cached = library_build(io)
    assert cached == {"root//:leaf": "Cache"}, cached
    source.write_bytes(original + b"\n")
    library_build(io)
    replace_trailer(driver, b"1")
    source.write_bytes(original)
    replaced = library_build(io)
    assert replaced == {"root//:leaf": "Local"}, replaced
    io.run(io.binary, "clean")
    restored = library_build(io)
    assert restored == {"root//:leaf": "Cache"}, restored


def rejects_external_driver(io: Io, driver: Path) -> None:
    """An escaping runtime symlink must fail before a warm result can be reused."""
    compiler = driver.parent.parent / "bin" / "rustc"
    version = io.run(str(compiler), "-vV").stdout
    external = io.project.parent / driver.name
    driver.rename(external)
    driver.symlink_to(external)
    assert io.run(str(compiler), "-vV").stdout == version
    result = io.run(io.binary, "build", ":leaf", succeeds=False)
    assert "unsupported system Rust toolchain" in result.stderr, result.stderr


def rejects_external_llvm(io: Io, driver: Path) -> None:
    """Loader-resolved compiler libraries outside the hashed closure must fail closed."""
    compiler = driver.parent.parent / "bin" / "rustc"
    version = io.run(str(compiler), "-vV").stdout
    data = driver.read_bytes()
    original = b"$ORIGIN/../lib\0"
    replacement = b"$ORIGIN/../..\0"
    assert data.count(original) == 1
    driver.write_bytes(data.replace(original, replacement.ljust(len(original), b"\0")))
    llvm = next(driver.parent.glob("libLLVM.so.*"))
    llvm.rename(io.project.parent / llvm.name)
    assert io.run(str(compiler), "-vV").stdout == version
    result = io.run(io.binary, "build", ":leaf", succeeds=False)
    assert "loaded library is not a hashed runtime file" in result.stderr, result.stderr


def main() -> None:
    """Run replacement regressions against a real Linux rustup installation."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("channel")
    parser.add_argument("prelude", type=Path, nargs="?")
    parser.add_argument("--case", choices=["compiler", "library", "layout", "llvm"], required=True)
    args = parser.parse_args()
    with tempfile.TemporaryDirectory() as directory:
        io = Io(binary=str(args.binary.resolve()), project=Path(directory) / "project")
        io.initialize(args.prelude.resolve() if args.prelude else None)
        toolchain = copied_toolchain(io, args.channel)
        compiler = toolchain / "bin" / "rustc"
        driver = next((toolchain / "lib").glob("librustc_driver-*.so"))
        for path in [compiler, driver]:
            with path.open("ab") as file:
                file.write(b"0")
        io = Io(
            binary=io.binary,
            project=io.project,
            search_path=f"{toolchain / 'bin'}:{io.search_path}",
        )
        try:
            if args.case == "compiler":
                replaces_compiler_before_memory_reuse(io, compiler)
            elif args.case == "library":
                library_build(io)
                replaces_library_before_disk_hit(io, driver)
            elif args.case == "layout":
                library_build(io)
                rejects_external_driver(io, driver)
            else:
                library_build(io)
                rejects_external_llvm(io, driver)
        finally:
            io.run(io.binary, "kill")
    print(f"ok  system Rust {args.case} replacement invalidates cached compiles")


if __name__ == "__main__":
    main()
