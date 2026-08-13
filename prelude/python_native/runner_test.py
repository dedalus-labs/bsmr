# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Verifies deterministic native Python action outputs.

"""Invariant tests for deterministic native Python action outputs."""

from __future__ import annotations

import csv
import os
import subprocess
import tempfile
import unittest
from argparse import Namespace
from pathlib import Path
from unittest.mock import patch

import runner


def _empty_environment(path: Path) -> Path:
    """Create the smallest valid BSMR environment tree for an action test."""
    path.mkdir()
    return path


class NormalizeEntryPointsTest(unittest.TestCase):
    """Exercise the relocatable console-script contract."""

    def test_shebang_and_record_are_normalized_together(self) -> None:
        """An absolute interpreter path must never survive in cached output."""
        with tempfile.TemporaryDirectory() as temporary:
            packages = Path(temporary)
            script = packages / "bin" / "demo"
            script.parent.mkdir()
            script.write_bytes(b"#!/tmp/action/python\nprint('demo')\n")
            record = packages / "demo-1.0.dist-info" / "RECORD"
            record.parent.mkdir()
            record.write_text("bin/demo,sha256=old,1\n", encoding="utf-8")
            (packages / ".lock").touch()

            runner._normalize_entry_points(packages)

            data = script.read_bytes()
            self.assertTrue(data.startswith(b"#!/usr/bin/env python3\n"))
            row = next(csv.reader(record.read_text(encoding="utf-8").splitlines()))
            self.assertEqual(
                row, ["bin/demo", runner._record_digest(data), str(len(data))]
            )
            self.assertFalse((packages / ".lock").exists())


class EnvironmentTest(unittest.TestCase):
    """Exercise composition of third- and first-party locked artifacts."""

    def test_first_party_wheels_are_installed_without_resolution(self) -> None:
        """Runtime metadata must come from exact wheel outputs, never editable sources."""
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            output = root / "environment.zip"
            wheel_directory = root / "wheel"
            wheel_directory.mkdir()
            wheel = wheel_directory / "demo-1-py3-none-any.whl"
            wheel.touch()
            args = Namespace(
                output=output,
                python=root / "python",
                uv=root / "uv",
                wheel_dir=[wheel_directory],
            )

            with patch.object(runner, "_run") as run:
                runner._wheel_environment(args, {"PATH": "/usr/bin"}, root / "scratch")

            install = run.call_args.args[0]
            self.assertIn(str(wheel.resolve()), install)
            self.assertIn("--no-deps", install)
            self.assertIn("--no-index", install)

    def test_source_builds_use_only_the_declared_build_environment(self) -> None:
        """A lock containing sdists must never resolve ambient build requirements."""
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            output = root / "environment"
            build_environment = _empty_environment(root / "build-environment")
            args = Namespace(
                build_environment=build_environment,
                config_setting=["--global-option=--quiet"],
                lock=root / "pylock.toml",
                output=output,
                python=root / "python",
                uv=root / "uv",
                wheel_dir=[],
            )
            process_environment = {"PATH": "/usr/bin"}

            with patch.object(runner, "_run") as run:
                runner._environment(args, process_environment, root / "scratch")

            command = run.call_args.args[0]
            self.assertIn("--no-build-isolation", command)
            self.assertNotIn("--no-build", command)
            self.assertIn("--config-setting=--global-option=--quiet", command)
            self.assertEqual(
                process_environment["PYTHONPATH"],
                str(build_environment.resolve()),
            )
            self.assertTrue(
                process_environment["PATH"].startswith(
                    str((build_environment / "bin").resolve())
                )
            )

    def test_environment_tree_rejects_symlinks(self) -> None:
        """A CAS tree must contain only unambiguous regular files and directories."""
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            packages = root / "packages"
            packages.mkdir()
            module = packages / "demo.py"
            module.write_text("VALUE = 1\n", encoding="utf-8")
            module.chmod(0o755)
            runner._validate_environment(packages)
            (packages / "link.py").symlink_to(module)
            with self.assertRaises(RuntimeError):
                runner._validate_environment(packages)


class ProjectActionTest(unittest.TestCase):
    """Exercise process boundaries shared by build and analysis actions."""

    def test_typecheck_uses_the_materialized_project_as_its_import_root(self) -> None:
        """Relative imports must resolve against the cached source tree."""
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "source"
            source.mkdir()
            environment = _empty_environment(root / "environment.zip")
            output = root / "typecheck.check"
            args = Namespace(
                environment=environment,
                config_setting=[],
                mode="ty",
                output=output,
                project_root=".",
                python=root / "python",
                source=source,
                ty=root / "ty",
                uv=None,
                ruff=None,
                vcs=None,
            )

            with patch.object(runner, "_run") as run:
                runner._project(args, {"PATH": "/usr/bin"}, root / "scratch")

            command = run.call_args.args[0]
            self.assertEqual(command[-1], ".")
            self.assertEqual(run.call_args.kwargs["cwd"], source.resolve())

    def test_tool_failure_propagates_without_a_wrapper_traceback(self) -> None:
        """The native tool owns diagnostics; the runner owns only its status."""
        completed = subprocess.CompletedProcess(["tool"], returncode=7)
        with (
            patch.object(runner.subprocess, "run", return_value=completed),
            self.assertRaises(SystemExit) as raised,
        ):
            runner._run(["tool"], {})

        self.assertEqual(raised.exception.code, 7)

    def test_wheel_uses_declared_project_configuration_without_ambient_state(
        self,
    ) -> None:
        """PEP 517 constraints are inputs while an ancestor checkout is not."""
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "source"
            source.mkdir()
            environment = _empty_environment(root / "environment.zip")
            vcs = root / "vcs"
            vcs.mkdir()
            (vcs / "HEAD").write_text("ref: refs/heads/main\n", encoding="utf-8")
            (root / "scratch").mkdir()
            args = Namespace(
                environment=environment,
                config_setting=["editable-mode=strict", "--build-option=--quiet"],
                mode="wheel",
                output=root / "wheel",
                project_root=".",
                python=root / "python",
                source=source,
                ty=None,
                uv=root / "uv",
                ruff=None,
                vcs=vcs,
            )
            process_environment = {"PATH": "/usr/bin", "UV_NO_CONFIG": "1"}

            with patch.object(runner, "_run") as run:
                runner._project(args, process_environment, root / "scratch")

            action_environment = run.call_args.args[1]
            self.assertEqual(action_environment["UV_NO_CONFIG"], "1")
            self.assertEqual(
                action_environment["GIT_CEILING_DIRECTORIES"],
                str(source.resolve().parent),
            )
            materialized_environment = environment.resolve()
            self.assertEqual(
                action_environment["PYTHONPATH"], str(materialized_environment)
            )
            self.assertTrue(
                action_environment["PATH"].startswith(
                    str(materialized_environment / "bin")
                )
            )
            self.assertIn("--no-build-isolation", run.call_args.args[0])
            self.assertEqual(
                run.call_args.args[0][-3:],
                [
                    "--config-setting=editable-mode=strict",
                    "--config-setting=--build-option=--quiet",
                    ".",
                ],
            )
            git_directory = root / "scratch" / "git"
            self.assertEqual(action_environment["GIT_DIR"], str(git_directory))
            self.assertEqual(
                (git_directory / "HEAD").read_text(encoding="utf-8"),
                "ref: refs/heads/main\n",
            )
            self.assertEqual(
                (git_directory / "config").read_text(encoding="utf-8"),
                "[core]\n\trepositoryformatversion = 0\n\tbare = false\n",
            )
            self.assertEqual(action_environment["GIT_WORK_TREE"], str(source.resolve()))
            self.assertEqual(action_environment["GIT_CONFIG_GLOBAL"], os.devnull)
            self.assertEqual(action_environment["GIT_CONFIG_NOSYSTEM"], "1")
            self.assertEqual(
                action_environment["GIT_INDEX_FILE"], str(git_directory / "index")
            )
            self.assertEqual(
                run.call_args_list[0].args[0], ["git", "read-tree", "HEAD"]
            )


if __name__ == "__main__":
    unittest.main()
