# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Verifies native Python runtime assembly.

"""Invariant tests for native Python runtime assembly."""

from __future__ import annotations

import os
import subprocess
import tempfile
import unittest
from argparse import Namespace
from pathlib import Path

import runtime


class RuntimeTest(unittest.TestCase):
    """Exercise workspace import-root and entry-point semantics."""

    def test_workspace_members_are_importable_without_installation(self) -> None:
        """A uv-style member must be visible through its standard project root."""
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "source"
            member = source / "packages" / "demo"
            member.mkdir(parents=True)
            (source / "pyproject.toml").write_text("[project]\n", encoding="utf-8")
            (member / "pyproject.toml").write_text("[project]\n", encoding="utf-8")
            environment = root / "environment"
            overlay = root / "overlay"
            environment.mkdir()
            overlay.mkdir()
            (environment / "dependency.py").touch()
            (overlay / "first_party.py").touch()
            args = Namespace(
                environment=[overlay, environment],
                project_root=".",
                source=source,
            )

            previous_directory = Path.cwd()
            previous_path = runtime.sys.path[:]
            previous_environment = os.environ.copy()
            try:
                scratch = root / "scratch"
                scratch.mkdir()
                runtime._bootstrap(args, scratch)

                self.assertEqual(Path.cwd(), source.resolve())
                self.assertEqual(
                    runtime.sys.path[:4],
                    [
                        str(source.resolve()),
                        str(member.resolve()),
                        str(overlay.resolve()),
                        str(environment.resolve()),
                    ],
                )
                self.assertEqual(os.environ["HOME"], str(root / "scratch" / "home"))
                declared = [
                    str(source.resolve()),
                    str(member.resolve()),
                    str(overlay.resolve()),
                    str(environment.resolve()),
                ]
                runtime_bin = (root / "scratch" / "bin").resolve()
                self.assertEqual(
                    os.environ["PATH"].split(os.pathsep)[:2],
                    [
                        str(runtime_bin),
                        str(overlay.resolve() / "bin"),
                    ],
                )
                self.assertEqual(os.environ["PYTHONPATH"].split(os.pathsep), declared)
                self.assertEqual(
                    (runtime_bin / "python").resolve(),
                    Path(runtime.sys.executable).resolve(),
                )
                self.assertEqual(
                    (runtime_bin / "python3").resolve(),
                    Path(runtime.sys.executable).resolve(),
                )
                child = subprocess.run(
                    [
                        str(runtime_bin / "python"),
                        "-c",
                        "import dependency, first_party",
                    ],
                    check=False,
                    capture_output=True,
                    env={"HOME": os.environ["HOME"], "PATH": os.environ["PATH"]},
                    text=True,
                )
                self.assertEqual(child.returncode, 0, child.stderr)
                self.assertFalse((source / ".bsmr-home").exists())
            finally:
                os.chdir(previous_directory)
                runtime.sys.path[:] = previous_path
                os.environ.clear()
                os.environ.update(previous_environment)

    def test_entry_point_requires_a_callable_module_object(self) -> None:
        """Malformed or non-callable entry points must fail before execution."""
        with self.assertRaises(ValueError):
            runtime._entry("demo")
        with self.assertRaises(TypeError):
            runtime._entry("sys:version")

    def test_environment_roots_must_be_materialized_directories(self) -> None:
        """Runtime must consume CAS trees directly and reject every other shape."""
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            environment = root / "environment"
            environment.mkdir()
            self.assertEqual(
                runtime._environment_root(environment), environment.resolve()
            )
            invalid = root / "environment.zip"
            invalid.touch()
            with self.assertRaises(RuntimeError):
                runtime._environment_root(invalid)


if __name__ == "__main__":
    unittest.main()
