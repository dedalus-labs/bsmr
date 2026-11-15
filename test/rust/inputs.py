# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

"""Qualify declared Rust source trees and generated inputs through the real engine."""

import argparse
import json
import os
import re
import shutil
import subprocess
import tempfile
from dataclasses import dataclass
from pathlib import Path

FIXTURE = Path(__file__).resolve().parent.parent / "fixtures" / "verified-inputs"


@dataclass(frozen=True, slots=True, kw_only=True)
class Io:
    """Own an initialized project and its private local cache."""

    binary: str
    project: Path

    def run(self, *arguments: str) -> subprocess.CompletedProcess[str]:
        """Invoke the engine with only this fixture's output and cache state."""
        env: dict[str, str] = {
            **os.environ,
            "BSMR_LOCAL_CACHE_DIR": str(self.project.parent / "cache"),
        }
        for key in ["RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTUP_TOOLCHAIN"]:
            env.pop(key, None)
        ret = subprocess.run(
            [self.binary, *arguments], cwd=self.project, env=env, capture_output=True, text=True
        )
        return ret

    def initialize(self, repository: Path, channel: str) -> None:
        """Vendor the checkout's current rules into a fresh initialized project."""
        self.project.mkdir()
        res = self.run("init")
        assert res.returncode == 0, res.stderr
        shutil.copytree(FIXTURE, self.project, dirs_exist_ok=True)
        shutil.copytree(repository / "prelude", self.project / "prelude")
        shutil.copytree(repository / "tools/build/rules", self.project / "build/rules")
        (self.project / "tree").mkdir()
        (self.project / "tree/value.txt").symlink_to(self.project / "value.txt")
        config = self.project / ".bsmr.local"
        config.write_text(
            config.read_text() + f"\n[fixture]\nundeclared_input = {self.project / 'value.txt'}\n"
        )
        (self.project / "rust-toolchain.toml").write_text(f'[toolchain]\nchannel="{channel}"\n')

    def build(self, target: str) -> dict[str, str]:
        """Require a successful build and return actual compiler executor records."""
        res = self.run("build", target, "--console", "simple", "--show-full-json-output")
        assert res.returncode == 0, res.stdout + res.stderr
        if target == ":app":
            output = next(iter(json.loads(res.stdout).values()))
            run = subprocess.run([output], capture_output=True, text=True)
            expected = (self.project / "value.txt").read_text().strip()
            assert run.returncode == 0, run.stderr
            assert run.stdout.strip() == f"{expected} {expected}", run.stdout
        trace = re.search(r"Build ID: ([a-f0-9-]+)", res.stderr)
        assert trace, res.stderr
        log = self.run(
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
        assert log.returncode == 0, log.stderr
        actions = [json.loads(line) for line in log.stdout.splitlines() if line.startswith("{")]
        ret = {row["identity"].split(" ")[0]: row["reproducer"]["executor"] for row in actions}
        return ret


def test_declared_inputs(io: Io) -> None:
    """Copied and generated sources must compile, restore and invalidate on content edits."""
    io.build(":copied")
    io.build(":app")
    res = io.run("clean")
    assert res.returncode == 0, res.stderr
    restored = io.build(":app")
    for target in ["root//:bindings", "root//:symbol", "root//:symbol@symbol"]:
        assert restored[target] == "Cache", restored
    (io.project / "value.txt").write_text("9\n")
    edited = io.build(":app")
    assert edited["root//:bindings"] == "Local", edited
    assert edited["root//:symbol@symbol"] == "Local", edited


def test_undeclared_inputs(io: Io) -> None:
    """Neither plain paths nor symlink escapes may extend the declared input trees."""
    for target in [":undeclared", ":symlink_escape"]:
        res = io.run("build", target, "--console", "simple")
        assert res.returncode != 0, f"accepted undeclared input for {target}"
        assert "undeclared Rust source input:" in res.stderr, res.stderr
        assert str(io.project / "value.txt") in res.stderr, res.stderr
    res = io.run("build", ":undeclared_env", "--console", "simple")
    assert res.returncode != 0, "accepted undeclared inherited environment"
    assert "undeclared Rust environment input: HOME" in res.stderr, res.stderr


def main() -> None:
    """Run the integration contract with a supplied engine and current rule checkout."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("repository", type=Path)
    parser.add_argument("channel")
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="bsmr-inputs-") as directory:
        io = Io(binary=str(args.binary.resolve()), project=Path(directory) / "project")
        io.initialize(repository=args.repository.resolve(), channel=args.channel)
        try:
            test_declared_inputs(io=io)
            test_undeclared_inputs(io=io)
        finally:
            res = io.run("kill")
            assert res.returncode == 0, res.stderr
    print("ok  Rust inputs: copied trees, generated macros, cache reuse, symlink rejection")


if __name__ == "__main__":
    main()
