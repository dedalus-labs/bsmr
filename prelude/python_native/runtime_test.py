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

    def test_only_the_requested_project_and_installed_wheels_are_importable(
        self,
    ) -> None:
        """Unselected workspace members must not leak into a runtime profile."""
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "source"
            member = source / "packages" / "demo"
            member.mkdir(parents=True)
            (source / "pyproject.toml").write_text("[project]\n", encoding="utf-8")
            (member / "pyproject.toml").write_text("[project]\n", encoding="utf-8")
            (member / "member_only.py").touch()
            environment = root / "environment"
            overlay = root / "overlay"
            environment.mkdir()
            overlay.mkdir()
            (environment / "dependency.py").touch()
            plugins = environment / "plugins"
            plugins.mkdir()
            (plugins / "pth_dependency.py").touch()
            (environment / "dependency.pth").write_text("plugins\n", encoding="utf-8")
            (overlay / "first_party.py").touch()
            args = Namespace(
                environment=[overlay, environment],
                project_root=".",
                source=source,
            )

            previous_directory = Path.cwd()
            previous_bytecode_policy = runtime.sys.dont_write_bytecode
            previous_path = runtime.sys.path[:]
            previous_environment = os.environ.copy()
            try:
                scratch = root / "scratch"
                scratch.mkdir()
                runtime._bootstrap(args, scratch)

                self.assertTrue(runtime.sys.dont_write_bytecode)
                self.assertEqual(Path.cwd(), source.resolve())
                self.assertEqual(
                    runtime.sys.path[:3],
                    [
                        str(source.resolve()),
                        str(overlay.resolve()),
                        str(environment.resolve()),
                    ],
                )
                self.assertEqual(os.environ["HOME"], str(root / "scratch" / "home"))
                declared = [
                    str(source.resolve()),
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
                        "import dependency, first_party, pth_dependency",
                    ],
                    check=False,
                    capture_output=True,
                    env={"HOME": os.environ["HOME"], "PATH": os.environ["PATH"]},
                    text=True,
                )
                self.assertEqual(child.returncode, 0, child.stderr)
                undeclared = subprocess.run(
                    [str(runtime_bin / "python"), "-c", "import member_only"],
                    check=False,
                    capture_output=True,
                    env={"HOME": os.environ["HOME"], "PATH": os.environ["PATH"]},
                    text=True,
                )
                self.assertNotEqual(undeclared.returncode, 0)
                self.assertFalse((source / ".bsmr-home").exists())
            finally:
                os.chdir(previous_directory)
                runtime.sys.dont_write_bytecode = previous_bytecode_policy
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

    def test_declared_test_command_uses_the_pinned_child_interpreter(self) -> None:
        """A non-pytest suite must remain inside the declared runtime."""
        runtime_bin = Path("/runtime/bin")

        self.assertEqual(
            runtime._test_command(
                runtime_bin,
                ["tests/runtests.py", "--verbosity", "1"],
                ["auth_tests"],
            ),
            [
                Path("/runtime/bin/python"),
                "tests/runtests.py",
                "--verbosity",
                "1",
                "auth_tests",
            ],
        )


if __name__ == "__main__":
    unittest.main()
