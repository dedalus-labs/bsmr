# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

"""Reuse system-toolchain compiles from the local cache only under the same compiler."""

import argparse
import json
import os
import re
import shutil
import subprocess
import tempfile
from dataclasses import dataclass
from pathlib import Path

FIXTURE = Path(__file__).resolve().parent.parent / "fixtures" / "system-rust"


@dataclass(frozen=True, slots=True, kw_only=True)
class Io:
    """Own one project, its private local cache and the engine under test."""

    binary: str
    project: Path
    search_path: str = os.environ["PATH"]

    def run(self, *arguments: str, succeeds: bool = True) -> subprocess.CompletedProcess[str]:
        """Run a command in the project and require its expected exit status."""
        env: dict[str, str] = {
            **os.environ,
            "PATH": self.search_path,
            "BSMR_LOCAL_CACHE_DIR": str(self.project.parent / "cache"),
        }
        for key in ["RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTUP_TOOLCHAIN"]:
            env.pop(key, None)
        res = subprocess.run(
            list(arguments), cwd=self.project, env=env, capture_output=True, text=True
        )
        assert (res.returncode == 0) == succeeds, res.stdout + res.stderr
        return res

    def initialize(self, prelude: Path | None) -> None:
        """Create a library and a binary that build with the system Rust toolchain."""
        self.project.mkdir()
        self.run(self.binary, "init")
        shutil.copytree(FIXTURE, self.project, dirs_exist_ok=True)
        if prelude is not None:
            shutil.copytree(prelude, self.project / "prelude")
            (self.project / ".bsmr.local").write_text("[external_cells]\nprelude = disabled\n")

    def build(self, channel: str) -> dict[str, str]:
        """Build with one rustup channel and return each compile's executor."""
        (self.project / "rust-toolchain.toml").write_text(f'[toolchain]\nchannel="{channel}"\n')
        res = self.run(
            self.binary, "build", ":app", "--console", "simple", "--show-full-json-output"
        )
        output = self.run(next(iter(json.loads(res.stdout).values())))
        assert output.stdout.strip() == "7", output.stdout
        return self.executors(res)

    def executors(self, result: subprocess.CompletedProcess[str]) -> dict[str, str]:
        """Read rustc executors for exactly the completed build's trace."""
        trace = re.search(r"Build ID: ([a-f0-9-]+)", result.stderr)
        assert trace, result.stderr
        log = self.run(
            self.binary,
            "log",
            "what-ran",
            "--trace-id",
            trace[1],
            "--format",
            "json",
            "--filter-category",
            "rustc",
            "--no-remote",
        )
        actions = [json.loads(line) for line in log.stdout.splitlines() if line.startswith("{")]
        ret = {
            action["identity"].split(" ")[0]: action["reproducer"]["executor"] for action in actions
        }
        return ret


def reuses_compiles_after_clean(io: Io, channel: str) -> None:
    """A clean must not discard compiles the local cache stored for the same compiler.

    Breaks if the system toolchain keeps its compiles out of the local cache.
    """
    cold = io.build(channel)
    assert cold == {"root//:leaf": "Local", "root//:app": "Local"}, cold
    io.run(io.binary, "clean")
    restored = io.build(channel)
    assert restored == {"root//:leaf": "Cache", "root//:app": "Local"}, restored


def misses_another_compiler(io: Io, channel: str, other: str) -> None:
    """A compile stored by one compiler must never serve a build with another.

    Breaks if the action key omits the compiler that rustc resolves to.
    """
    switched = io.build(other)
    assert switched == {"root//:leaf": "Local", "root//:app": "Local"}, switched
    restored = io.build(channel)
    assert restored == {"root//:leaf": "Cache", "root//:app": "Local"}, restored


def main() -> None:
    """Qualify the supplied engine with two installed rustup channels."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("channel")
    parser.add_argument("other")
    parser.add_argument("prelude", type=Path, nargs="?")
    args = parser.parse_args()
    with tempfile.TemporaryDirectory() as directory:
        io = Io(binary=str(args.binary.resolve()), project=Path(directory) / "project")
        io.initialize(prelude=args.prelude.resolve() if args.prelude else None)
        try:
            reuses_compiles_after_clean(io=io, channel=args.channel)
            misses_another_compiler(io=io, channel=args.channel, other=args.other)
            io.run(io.binary, "build", ":probe-run[out_dir]")
            rejected = io.run(io.binary, "build", ":loader-override", succeeds=False)
            assert "loader override is set: LD_BIND_NOW" in rejected.stderr, rejected.stderr
            if args.channel.startswith("nightly"):
                io.run(io.binary, "test", ":leaf[doc]")
            for case in ["compiler", "library", "layout", "llvm"]:
                io.run(
                    sys.executable,
                    str(Path(__file__).with_name("system_identity.py").resolve()),
                    io.binary,
                    args.channel,
                    *([str(args.prelude.resolve())] if args.prelude else []),
                    "--case",
                    case,
                )
        finally:
            io.run(io.binary, "kill")
    print("ok  system rust toolchain: clean reuse, compiler identity")


if __name__ == "__main__":
    main()
