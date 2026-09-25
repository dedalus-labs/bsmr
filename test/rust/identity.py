# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

"""Compare a build script's checkout identity with Git through real source changes."""

import argparse
import tempfile
from dataclasses import dataclass
from pathlib import Path

from macros import build, run

SCRIPT = r"""
use std::process::Command;

fn git(arguments: &[&str]) -> String {
    let output = Command::new("git").args(arguments).output().unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8(output.stdout).unwrap()
}

fn main() {
    let revision = git(&["rev-parse", "HEAD"]);
    let dirty = !git(&["status", "--porcelain", "--untracked-files=normal"]).is_empty();
    let files = git(&[
        "ls-files", "--full-name", "-z", "--cached", "--others", "--exclude-standard", "--", ":/",
    ]);
    println!("cargo::rustc-env=SOURCE_REVISION={}", revision.trim());
    println!("cargo::rustc-env=SOURCE_DIRTY={dirty}");
    println!("cargo::rustc-env=SOURCE_FILES={}", files.replace('\0', ";"));
}
"""


@dataclass(frozen=True, slots=True, kw_only=True)
class Io:
    """Own the compiler, runtime and source checkouts for this qualification."""

    binary: str
    runtime: Path

    def git(self, directory: Path, *arguments: str) -> str:
        """Inspect or advance only the fixture's Git state."""
        result = run(
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

    def initialize(self, project: Path) -> None:
        """Select the same verified runtime for each independent checkout."""
        result = run(project, self.binary, "init")
        assert result.returncode == 0, result.stderr
        (project / ".bsmr.local").write_text(
            "[bsmr]\ndefault_allow_cache_upload=true\n"
            f"[sandbox]\nbackend=namespace\nruntime={self.runtime}\n"
        )

    def seed(self, project: Path) -> None:
        """Create a nested package with tracked hidden files and a relative link."""
        files = {
            "Cargo.toml": '[workspace]\nmembers=["app"]\nresolver="2"\n',
            "rust-toolchain.toml": '[toolchain]\nchannel="1.97.1"\n',
            ".gitignore": "/.bsmr\n/.bsmr.local\n/bsmr-out/\n/target/\n",
            ".settings/value": "tracked hidden data\n",
            "app/Cargo.toml": '[package]\nname="app"\nversion="0.1.0"\nedition="2024"\n',
            "app/build.rs": SCRIPT,
            "app/src/main.rs": (
                'fn main() { println!("{} {}\\n{}", env!("SOURCE_REVISION"), '
                'env!("SOURCE_DIRTY"), env!("SOURCE_FILES")); }\n'
            ),
        }
        for name, text in files.items():
            path = project / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text)
        (project / "settings").symlink_to("./.settings", target_is_directory=True)
        result = run(project, "cargo", "generate-lockfile", "--offline")
        assert result.returncode == 0, result.stderr
        self.git(project, "init", "-b", "main")
        self.git(project, "add", ".")
        self.git(project, "commit", "-m", "fixture")
        revision = self.git(project, "rev-parse", "HEAD")
        (project / "vendor/empty").mkdir(parents=True)
        self.git(project, "update-index", "--add", "--cacheinfo", f"160000,{revision},vendor/empty")
        self.git(project, "commit", "-m", "track uninitialized submodule")
        self.initialize(project)

    def check(self, project: Path) -> list[str]:
        """Require the compiled result to describe the same HEAD, dirtiness and file list."""
        revision = self.git(project, "rev-parse", "HEAD")
        status = self.git(project, "status", "--porcelain", "--untracked-files=normal")
        files = self.git(
            project,
            "ls-files",
            "--full-name",
            "-z",
            "--cached",
            "--others",
            "--exclude-standard",
            "--",
            ":/",
        ).replace("\0", ";")
        expected = f"{revision} {str(bool(status)).lower()}\n{files}"
        actions = build(project, self.binary, expected)
        return actions

    def qualify(self, root: Path) -> None:
        """Check persistent-daemon invalidation and a linked worktree's private index."""
        source, linked = root / "source", root / "linked"
        self.seed(source)
        try:
            assert self.check(source), "cold build must compile"
            assert self.check(source) == [], "unchanged identity must reuse compilation"
            (source / "vendor/empty").rmdir()
            self.check(source)
            (source / "vendor/empty").mkdir()
            self.check(source)
            (source / "new.txt").write_text("untracked input\n")
            self.check(source)
            self.git(source, "add", "new.txt")
            self.check(source)
            self.git(source, "commit", "-m", "advance source")
            self.check(source)
            self.git(source, "commit", "--allow-empty", "-m", "advance metadata")
            self.check(source)
            self.git(source, "pack-refs", "--all", "--prune")
            self.check(source)
            self.git(source, "worktree", "add", "--detach", str(linked), "HEAD")
            self.initialize(linked)
            self.check(linked)
            (linked / "linked.txt").write_text("private linked change\n")
            self.git(linked, "add", "linked.txt")
            self.check(linked)
            self.check(source)
            print("ok: checkout identity, hidden files, symlinks, linked worktrees, warm reuse")
        finally:
            for project in (source, linked):
                if (project / ".bsmr").exists():
                    stopped = run(project, self.binary, "kill")
                    assert stopped.returncode == 0, stopped.stderr


def main() -> None:
    """Own temporary checkouts until all compiler processes have stopped."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("runtime", type=Path)
    arguments = parser.parse_args()
    io = Io(binary=str(arguments.binary.resolve()), runtime=arguments.runtime.resolve())
    with tempfile.TemporaryDirectory(prefix="bsmr-identity-") as directory:
        io.qualify(Path(directory))


if __name__ == "__main__":
    main()
