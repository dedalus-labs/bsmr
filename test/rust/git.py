# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

"""Qualify locked Git sources independently of compiler network access."""

import argparse
import json
import tempfile
from dataclasses import dataclass
from pathlib import Path
from subprocess import CompletedProcess

from macros import build, run


@dataclass(frozen=True, slots=True, kw_only=True)
class Io:
    """Own one Git origin, consumer and compiler cache for the test."""

    root: Path
    binary: str
    runtime: Path

    def run(self, directory: Path, *arguments: str) -> CompletedProcess[str]:
        """Run a bounded command without changing shared compiler processes."""
        result = run(directory, *arguments)
        return result

    def git(self, directory: Path, *arguments: str) -> str:
        """Run fixture Git with an explicit commit identity."""
        result = self.run(
            directory,
            "git",
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "-c",
            "commit.gpgsign=false",
            *arguments,
        )
        assert result.returncode == 0, result.stderr
        return result.stdout.strip()

    def seed(self) -> Path:
        """Pin a nested package whose original checkout is outside the sandbox."""
        origin = self.root / "origin with spaces"
        library = origin / "crates/value"
        (library / "src").mkdir(parents=True)
        (origin / "Cargo.toml").write_text('[workspace]\nmembers=["crates/value"]\nresolver="2"\n')
        (library / "Cargo.toml").write_text(
            '[package]\nname="value"\nversion="0.1.0"\nedition="2024"\n'
        )
        (library / "src/lib.rs").write_text("pub fn value() -> u8 { 17 }\n")
        self.git(origin, "init", "-b", "main")
        self.git(origin, "add", ".")
        self.git(origin, "commit", "-m", "fixture")
        revision = self.git(origin, "rev-parse", "HEAD")
        project = self.root / "consumer"
        (project / "app/src").mkdir(parents=True)
        (project / "Cargo.toml").write_text('[workspace]\nmembers=["app"]\nresolver="2"\n')
        (project / "app/Cargo.toml").write_text(
            '[package]\nname="app"\nversion="0.1.0"\nedition="2024"\n'
            f'[dependencies]\nvalue={{git={json.dumps(origin.as_uri())},rev="{revision}"}}\n'
        )
        (project / "app/src/main.rs").write_text('fn main() { println!("{}", value::value()); }\n')
        (project / "rust-toolchain.toml").write_text('[toolchain]\nchannel="1.97.1"\n')
        locked = self.run(project, "cargo", "generate-lockfile")
        assert locked.returncode == 0, locked.stderr
        initialized = self.run(project, self.binary, "init")
        assert initialized.returncode == 0, initialized.stderr
        (project / ".bsmr.local").write_text(
            "[bsmr]\ndefault_allow_cache_upload=true\n"
            f"[sandbox]\nbackend=namespace\nruntime={self.runtime}\n"
        )
        (library / "src/lib.rs").write_text('compile_error!("mutable origin must not compile");\n')
        return project

    def qualify(self) -> None:
        """Require correct cold execution, warm reuse and origin-independent restoration."""
        project = self.seed()
        lock = (project / "Cargo.lock").read_bytes()
        try:
            assert build(project, self.binary, "17"), "cold build must compile"
            assert build(project, self.binary, "17") == [], "warm build must reuse"
            self.stop(project)
            origin = self.root / "origin with spaces"
            origin.rename(self.root / "unavailable")
            # Removing the daemon forces a fresh planner process against its acquired objects.
            actions = build(project, self.binary, "17")
            assert all(
                json.loads(action)["reproducer"]["executor"] == "Cache" for action in actions
            )
            assert (project / "Cargo.lock").read_bytes() == lock
            print("ok: locked Git source, sandboxed compilation, warm reuse, unavailable origin")
        finally:
            self.stop(project)

    def stop(self, project: Path) -> None:
        """Stop only this test's daemon before releasing its files."""
        result = self.run(project, self.binary, "kill")
        assert result.returncode == 0, result.stderr


def main() -> None:
    """Select a verified engine/runtime pair and own the fixture until cleanup."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("runtime", type=Path)
    args = parser.parse_args()
    with tempfile.TemporaryDirectory(prefix="bsmr-git-") as directory:
        io = Io(
            root=Path(directory), binary=str(args.binary.resolve()), runtime=args.runtime.resolve()
        )
        io.qualify()


if __name__ == "__main__":
    main()
