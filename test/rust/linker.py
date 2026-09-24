# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

"""Compare selected linkers with Cargo in a pinned execution runtime."""

import argparse
import json
import tempfile
from dataclasses import dataclass
from pathlib import Path
from subprocess import CompletedProcess

from macros import build, run


@dataclass(frozen=True, slots=True, kw_only=True)
class Linker:
    """A compiler driver and the argument selecting its linker."""

    driver: str
    argument: str

    def config(self) -> str:
        """Render the exact selection as Cargo configuration."""
        flags = json.dumps(["-C", "link-arg=" + self.argument])
        config = (
            "[target.'cfg(target_os = \"linux\")']\n"
            f"linker={json.dumps(self.driver)}\n"
            f"rustflags={flags}\n"
        )
        return config


@dataclass(frozen=True, slots=True, kw_only=True)
class Io:
    """Own one temporary project and its compiler processes."""

    project: Path
    binary: str
    runtime: Path

    def run(self, *arguments: str) -> CompletedProcess[str]:
        """Run one bounded command in this project's private cache."""
        result = run(self.project, *arguments)
        return result

    def configure(self, *, linker: Linker) -> None:
        """Replace the project's selected driver and linker argument."""
        config = self.project / ".cargo/config.toml"
        config.write_text(linker.config())

    def initialize(self, *, linker: Linker) -> None:
        """Create the Cargo reference and native fixture from identical inputs."""
        files = {
            "Cargo.toml": '[workspace]\nmembers=["app"]\nresolver="2"\n',
            "rust-toolchain.toml": '[toolchain]\nchannel="1.97.1"\n',
            "app/Cargo.toml": ('[package]\nname="app"\nversion="0.1.0"\nedition="2024"\n'),
            "app/src/main.rs": 'fn main() { println!("17"); }\n',
            "app/linker.txt": linker.driver,
            "app/build.rs": (
                "fn main() {\n"
                '    let expected = std::fs::read_to_string("linker.txt").unwrap();\n'
                '    assert_eq!(std::env::var("RUSTC_LINKER").unwrap(), expected);\n'
                "}\n"
            ),
        }
        for name, text in files.items():
            path = self.project / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text(text)
        config = self.project / ".cargo"
        config.mkdir()
        self.configure(linker=linker)
        reference = self.run("cargo", "generate-lockfile", "--offline")
        assert reference.returncode == 0, reference.stderr
        reference = self.run("cargo", "run", "--locked", "--offline", "-p", "app")
        assert reference.returncode == 0, reference.stderr
        assert reference.stdout.strip() == "17", reference.stdout
        initialized = self.run(self.binary, "init")
        assert initialized.returncode == 0, initialized.stderr
        config = self.project / ".bsmr.local"
        config.write_text(
            "[bsmr]\ndefault_allow_cache_upload=true\n"
            f"[sandbox]\nbackend=namespace\nruntime={self.runtime}\n"
        )

    def build(self) -> list[str]:
        """Build and run the fixture, returning its compiler execution records."""
        actions = build(project=self.project, binary=self.binary, expected="17")
        return actions

    def qualify(self, *, linker: Linker) -> None:
        """Check that the requested tools determine execution and cache reuse."""
        assert self.build(), "cold build must invoke the linker"
        assert self.build() == [], "unchanged build must reuse actions"
        unavailable = (
            Linker(driver=linker.driver, argument="-fuse-ld=unavailable-bsmr-test-linker"),
            Linker(driver="unavailable-bsmr-test-driver", argument=linker.argument),
        )
        for selected in unavailable:
            self.configure(linker=selected)
            failed = self.run(self.binary, "build", "app", "--sandbox", "--console", "simple")
            assert failed.returncode != 0, "a missing tool must fail without substitution"
            assert "unavailable-bsmr-test" in failed.stderr, failed.stderr
        self.configure(linker=linker)
        restored = self.build()
        for action in restored:
            assert json.loads(action)["reproducer"]["executor"] == "Cache", action
        rejected = self.run(self.binary, "build", "app", "--console", "simple")
        assert rejected.returncode != 0, rejected.stderr
        assert "verified declared-input executor" in rejected.stderr, rejected.stderr
        print("ok: configured tools, script environment and cache isolation")

    def stop(self) -> None:
        """Reap the fixture's daemon before its temporary directory disappears."""
        config = self.project / ".bsmr"
        if not config.exists():
            return
        stopped = self.run(self.binary, "kill")
        assert stopped.returncode == 0, stopped.stderr


def main() -> None:
    """Parse the selected tools and own the fixture until cleanup completes."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("runtime", type=Path)
    parser.add_argument("driver")
    parser.add_argument("argument")
    args = parser.parse_args()
    binary = str(args.binary.resolve(strict=True))
    runtime = args.runtime.resolve(strict=True)
    linker = Linker(driver=args.driver, argument=args.argument)
    with tempfile.TemporaryDirectory(prefix="bsmr-linker-") as temporary:
        project = Path(temporary) / "project"
        project.mkdir()
        io = Io(project=project, binary=binary, runtime=runtime)
        try:
            io.initialize(linker=linker)
            io.qualify(linker=linker)
        finally:
            io.stop()


if __name__ == "__main__":
    main()
