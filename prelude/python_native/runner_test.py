# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Verifies deterministic native Python action outputs.

"""Invariant tests for deterministic native Python action outputs."""

from __future__ import annotations

import csv
import json
import os
import runpy
import subprocess
import sys
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


def _package_with_import_metadata(
    root: Path, distribution: str, metadata: str, module: str
) -> tuple[Path, Path]:
    """Create one installed distribution with explicit PEP 794 ownership."""
    package = root / distribution
    package.mkdir()
    (package / module).write_text(f"OWNER = {distribution!r}\n", encoding="utf-8")
    metadata_file = package / f"{distribution}-1.dist-info" / "METADATA"
    metadata_file.parent.mkdir()
    metadata_file.write_text(
        f"Metadata-Version: 2.5\nName: {distribution}\nVersion: 1\n{metadata}\n",
        encoding="utf-8",
    )
    manifest = root / f"{distribution}.json"
    runner._write_package_manifest(package, manifest)
    return package, manifest


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
            self.assertNotIn(b"/tmp/action/python", data)
            row = next(csv.reader(record.read_text(encoding="utf-8").splitlines()))
            self.assertEqual(
                row, ["bin/demo", runner._record_digest(data), str(len(data))]
            )
            self.assertFalse((packages / ".lock").exists())

    def test_uv_shell_trampoline_is_removed(self) -> None:
        """uv's absolute interpreter trampoline must not enter a CAS artifact."""
        with tempfile.TemporaryDirectory() as temporary:
            packages = Path(temporary)
            script = packages / "bin" / "demo"
            script.parent.mkdir()
            script.write_bytes(
                b"#!/tmp/action/python\n"
                b"'''exec' '/tmp/action/python' \"$0\" \"$@\"\n"
                b"' '''\n"
                b"from demo import main\nmain()\n"
            )
            record = packages / "demo-1.0.dist-info" / "RECORD"
            record.parent.mkdir()
            record.write_text("bin/demo,sha256=old,1\n", encoding="utf-8")

            runner._normalize_entry_points(packages)

            data = script.read_bytes()
            self.assertEqual(
                data,
                b"#!/usr/bin/env python3\nfrom demo import main\nmain()\n",
            )
            row = next(csv.reader(record.read_text(encoding="utf-8").splitlines()))
            self.assertEqual(
                row, ["bin/demo", runner._record_digest(data), str(len(data))]
            )


class SysconfigTest(unittest.TestCase):
    """Exercise relocation of python-build-standalone compiler metadata."""

    def test_native_builds_disable_path_dependent_debug_sections(self) -> None:
        """Cold actions in different scratch roots must produce identical binaries."""
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with patch.dict(
                os.environ, {"BUCK_SCRATCH_PATH": str(root / "scratch")}, clear=True
            ):
                _, environment = runner._state(root / "output")

            self.assertEqual(environment["CFLAGS"], "-g0")
            self.assertEqual(environment["CXXFLAGS"], "-g0")

    def test_target_platform_configures_one_canonical_macos_deployment(self) -> None:
        """Wheel selection and PEP 517 must share the declared Darwin baseline."""
        with tempfile.TemporaryDirectory() as temporary:
            environment = {"MACOSX_DEPLOYMENT_TARGET": "host"}

            runner._configure_target_platform(
                "aarch64-apple-darwin", environment, Path(temporary)
            )

            self.assertEqual(environment["MACOSX_DEPLOYMENT_TARGET"], "13.0")
            self.assertEqual(environment["_PYTHON_HOST_PLATFORM"], "macosx-13.0-arm64")
            output = subprocess.check_output(
                [
                    sys.executable,
                    "-c",
                    "import platform; print(platform.mac_ver()[0]); print(platform.mac_ver()[2])",
                ],
                env=environment,
                text=True,
            )
            self.assertEqual(output, "13.0.0\narm64\n")

            linux_environment = {
                "MACOSX_DEPLOYMENT_TARGET": "host",
                "_PYTHON_HOST_PLATFORM": "host",
            }
            runner._configure_target_platform(
                "aarch64-unknown-linux-gnu", linux_environment, Path(temporary)
            )
            self.assertNotIn("MACOSX_DEPLOYMENT_TARGET", linux_environment)
            self.assertNotIn("_PYTHON_HOST_PLATFORM", linux_environment)

    def test_build_metadata_uses_the_materialized_interpreter_and_host_tools(
        self,
    ) -> None:
        """PEP 517 builds must not retain the archive producer's paths."""
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "python"
            python = root / "bin" / "python3"
            python.parent.mkdir(parents=True)
            python.touch()
            data = root / "lib" / "python3.14" / "_sysconfigdata__darwin.py"
            data.parent.mkdir(parents=True)
            data.write_text(
                "build_time_vars = {"
                "'AR': '/tmp/build/tools/llvm/bin/llvm-ar', "
                "'CC': 'clang -pthread', "
                "'CXX': 'clang++ -pthread', "
                "'LDSHARED': 'clang -bundle -isysroot /tmp/MacOSX.sdk', "
                "'prefix': '/install', "
                "'INCLUDEPY': '/install/include/python3.14'"
                "}\n",
                encoding="utf-8",
            )
            scratch = Path(temporary) / "scratch"
            scratch.mkdir()
            environment = {"PYTHONPATH": "/declared/packages"}

            runner._configure_sysconfig(python, scratch, environment)

            patched = runpy.run_path(
                str(scratch / "sysconfig" / "_bsmr_sysconfigdata.py")
            )["build_time_vars"]
            self.assertEqual(patched["AR"], "ar")
            self.assertEqual(patched["CC"], "cc -pthread")
            self.assertEqual(patched["CXX"], "c++ -pthread")
            self.assertEqual(patched["LDSHARED"], "cc -bundle")
            self.assertEqual(patched["prefix"], str(root.resolve()))
            self.assertEqual(
                patched["INCLUDEPY"],
                str(root.resolve() / "include" / "python3.14"),
            )
            self.assertEqual(patched["PYTHON_BUILD_STANDALONE"], 1)
            self.assertEqual(
                environment["_PYTHON_SYSCONFIGDATA_NAME"], "_bsmr_sysconfigdata"
            )
            self.assertEqual(
                environment["_PYTHON_SYSCONFIGDATA_PATH"],
                str(scratch / "sysconfig"),
            )
            self.assertEqual(environment["PYTHONPATH"], "/declared/packages")

    def test_unknown_compiler_metadata_fails_closed(self) -> None:
        """A new standalone toolchain layout requires an explicit BSMR update."""
        with self.assertRaisesRegex(RuntimeError, "unsupported CC value"):
            runner._patch_sysconfig_values({"CC": "/mystery/compiler"}, Path("/p"))


class EnvironmentTest(unittest.TestCase):
    """Exercise composition of third- and first-party locked artifacts."""

    def test_uv_selects_a_compatible_locked_wheel(self) -> None:
        """A successful binary-only dry run commits the remote-cacheable path."""
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            args = Namespace(
                distribution="attrs",
                lock=root / "pylock.toml",
                output=root / "selection",
                python=root / "python",
                python_platform="aarch64-apple-darwin",
                source_permitted=False,
                uv=root / "uv",
            )
            completed = subprocess.CompletedProcess(
                [],
                returncode=0,
                stdout="",
                stderr="Would install 1 package\n + attrs==25.4.0\n",
            )

            with patch.object(runner.subprocess, "run", return_value=completed) as run:
                runner._select_locked_package(args, {}, root / "scratch")

            self.assertEqual(
                json.loads(args.output.read_text(encoding="utf-8")),
                {"acquisition": "wheel", "requirement": "attrs==25.4.0"},
            )
            command = run.call_args.args[0]
            self.assertIn("--dry-run", command)
            self.assertIn("--offline", command)
            self.assertEqual(
                command[command.index("--python-platform") + 1],
                "aarch64-apple-darwin",
            )
            self.assertEqual(command[command.index("--only-binary") + 1], ":all:")
            self.assertEqual(command.count("--preview-features"), 1)
            self.assertEqual(command[command.index("--preview-features") + 1], "pylock")

    def test_uv_selects_locked_source_when_no_wheel_is_compatible(self) -> None:
        """Only uv's pinned no-binary result may commit the local source path."""
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            args = Namespace(
                distribution="pyarrow",
                lock=root / "pylock.toml",
                output=root / "selection",
                python=root / "python",
                python_platform="aarch64-apple-darwin",
                source_permitted=True,
                uv=root / "uv",
            )
            completed = subprocess.CompletedProcess(
                [],
                returncode=2,
                stdout="",
                stderr=(
                    "Using Python\n"
                    "error: Package `pyarrow` can't be installed because it is marked "
                    "as `--no-build` but has no binary distribution\n"
                ),
            )

            with patch.object(runner.subprocess, "run", return_value=completed):
                runner._select_locked_package(args, {}, root / "scratch")

            self.assertEqual(
                json.loads(args.output.read_text(encoding="utf-8")),
                {"acquisition": "source"},
            )

            args.output.unlink()
            args.source_permitted = False
            with (
                patch.object(runner.subprocess, "run", return_value=completed),
                patch.object(runner.sys.stderr, "write"),
                self.assertRaises(SystemExit),
            ):
                runner._select_locked_package(args, {}, root / "scratch")
            self.assertFalse(args.output.exists())

    def test_uv_selects_absent_marker_variant(self) -> None:
        """A package excluded by exact lock markers becomes an empty CAS tree."""
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            args = Namespace(
                distribution="exceptiongroup",
                lock=root / "pylock.toml",
                output=root / "selection",
                python=root / "python",
                python_platform="aarch64-apple-darwin",
                source_permitted=True,
                uv=root / "uv",
            )
            completed = subprocess.CompletedProcess(
                [], returncode=0, stdout="", stderr="Would install 0 packages\n"
            )

            with patch.object(runner.subprocess, "run", return_value=completed):
                runner._select_locked_package(args, {}, root / "scratch")

            self.assertEqual(
                json.loads(args.output.read_text(encoding="utf-8")),
                {"acquisition": "absent"},
            )

    def test_uv_selection_failure_cannot_be_misclassified_as_source(self) -> None:
        """Network, lock, and interpreter failures must remain uv failures."""
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            args = Namespace(
                distribution="demo",
                lock=root / "pylock.toml",
                output=root / "selection",
                python=root / "python",
                python_platform="aarch64-apple-darwin",
                source_permitted=False,
                uv=root / "uv",
            )
            completed = subprocess.CompletedProcess(
                [], returncode=2, stdout="", stderr="error: corrupted lock\n"
            )

            with (
                patch.object(runner.subprocess, "run", return_value=completed),
                patch.object(runner.sys.stderr, "write"),
                self.assertRaises(SystemExit) as raised,
            ):
                runner._select_locked_package(args, {}, root / "scratch")

            self.assertEqual(raised.exception.code, 2)
            self.assertFalse(args.output.exists())

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
                python_platform="aarch64-apple-darwin",
                uv=root / "uv",
                wheel_dir=[wheel_directory],
            )

            def install(_: list[str], __: dict[str, str]) -> None:
                (output / "demo.py").write_text("VALUE = 1\n", encoding="utf-8")

            with patch.object(runner, "_run", side_effect=install) as run:
                runner._wheel_environment(args, {"PATH": "/usr/bin"}, root / "scratch")

            install = run.call_args.args[0]
            self.assertIn(str(wheel.resolve()), install)
            self.assertIn("--no-deps", install)
            self.assertIn("--no-index", install)
            self.assertIn("--no-python-downloads", install)
            self.assertIn("--offline", install)
            self.assertEqual(
                install[install.index("--python-platform") + 1],
                "aarch64-apple-darwin",
            )

    def test_source_builds_use_only_the_declared_build_environment(self) -> None:
        """A lock containing sdists must never resolve ambient build requirements."""
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            output = root / "environment"
            build_environment = _empty_environment(root / "build-environment")
            args = Namespace(
                absent=False,
                artifact=[],
                build_environment=[build_environment],
                config_setting=["--global-option=--quiet"],
                distribution=None,
                package_config_setting=["numpy:setup-args=-Dblas=blas"],
                package_build_variable=["numpy:NPY_DISABLE_CPU_FEATURES=AVX512"],
                lock=root / "pylock.toml",
                manifest=root / "package.json",
                output=output,
                python=root / "python",
                python_platform="aarch64-apple-darwin",
                requirement=None,
                source_artifact=None,
                source_subdirectory=None,
                source_tree=None,
                uv=root / "uv",
                version=None,
                wheel_dir=[],
            )
            process_environment = {"PATH": "/usr/bin"}

            with (
                patch.object(runner, "_run") as run,
                patch.object(
                    runner,
                    "_validate_environment",
                    wraps=runner._validate_environment,
                ) as validate,
            ):
                (root / "scratch").mkdir()
                runner._locked_package(args, process_environment, root / "scratch")

            validate.assert_called_once_with(output.resolve())
            command = run.call_args.args[0]
            self.assertIn("--no-build-isolation", command)
            self.assertNotIn("--no-build", command)
            self.assertEqual(
                command[command.index("--python-platform") + 1],
                "aarch64-apple-darwin",
            )
            self.assertIn("--config-setting=--global-option=--quiet", command)
            self.assertIn(
                "--config-settings-package=numpy:setup-args=-Dblas=blas", command
            )
            config = Path(command[command.index("--config-file") + 1])
            self.assertEqual(
                config.read_text(encoding="utf-8"),
                '[extra-build-variables."numpy"]\n'
                '"NPY_DISABLE_CPU_FEATURES" = "AVX512"\n',
            )
            python_path = process_environment["PYTHONPATH"].split(os.pathsep)
            self.assertEqual(python_path[0], str(root / "scratch" / "target-platform"))
            self.assertEqual(python_path[1], str(build_environment.resolve()))
            self.assertTrue(
                process_environment["PATH"].startswith(
                    str((build_environment / "bin").resolve())
                )
            )

    def test_source_builds_install_the_verified_artifact_offline(self) -> None:
        """PEP 517 must consume acquired bytes without lock or index access."""
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            artifact = root / "demo-1.tar.gz"
            artifact.touch()
            args = Namespace(
                absent=False,
                artifact=[],
                build_environment=[_empty_environment(root / "build-environment")],
                config_setting=[],
                distribution="demo",
                package_config_setting=[],
                package_build_variable=[],
                lock=root / "pylock.toml",
                manifest=root / "package.json",
                output=root / "environment",
                python=root / "python",
                python_platform="aarch64-apple-darwin",
                requirement=None,
                source_artifact=artifact,
                source_subdirectory=None,
                source_tree=None,
                uv=root / "uv",
                version="1",
            )

            def install(*_: object) -> None:
                metadata = args.output / "demo-1.dist-info" / "METADATA"
                metadata.parent.mkdir()
                metadata.write_text("Name: demo\nVersion: 1\n", encoding="utf-8")

            with patch.object(runner, "_run", side_effect=install) as run:
                (root / "scratch").mkdir()
                runner._locked_package(args, {"PATH": "/usr/bin"}, root / "scratch")

            command = run.call_args.args[0]
            self.assertEqual(command[1:4], ["pip", "install", str(artifact.resolve())])
            self.assertNotIn(str(args.lock), command)
            self.assertIn("--no-build-isolation", command)
            self.assertNotIn("--no-build", command)
            self.assertIn("--no-deps", command)
            self.assertIn("--no-index", command)
            self.assertIn("--offline", command)

    def test_source_builds_reject_mismatched_distribution_metadata(self) -> None:
        """An archive may not install a different project than its lock entry."""
        with tempfile.TemporaryDirectory() as temporary:
            packages = Path(temporary)
            metadata = packages / "other-2.dist-info" / "METADATA"
            metadata.parent.mkdir()
            metadata.write_text("Name: other\nVersion: 2\n", encoding="utf-8")

            with self.assertRaisesRegex(
                RuntimeError,
                "expected demo==1, installed other==2",
            ):
                runner._validate_distribution_identity(packages, "demo", "1")

    def test_source_archive_subdirectory_is_encoded_without_changing_path(self) -> None:
        """A locked project root remains data inside one local file URL."""
        with tempfile.TemporaryDirectory() as temporary:
            artifact = Path(temporary) / "source archive.zip"
            artifact.touch()

            reference = runner._source_artifact_reference(
                artifact, "python/package name"
            )

            self.assertEqual(
                reference,
                f"{artifact.resolve().as_uri()}#subdirectory=python/package%20name",
            )

    def test_source_tree_subdirectory_is_contained_by_the_acquired_root(self) -> None:
        """A VCS project root resolves inside its declared CAS tree."""
        with tempfile.TemporaryDirectory() as temporary:
            tree = Path(temporary)
            source = tree / "python" / "package"
            source.mkdir(parents=True)

            self.assertEqual(
                runner._source_tree_reference(tree, "python/package"),
                str(source.resolve()),
            )
            with self.assertRaisesRegex(ValueError, "normalized relative path"):
                runner._source_tree_reference(tree, "../package")

    def test_locked_wheel_installs_from_its_verified_artifact(self) -> None:
        """A directly acquired wheel must not perform index or lock resolution."""
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            artifact = root / "demo-1-py3-none-any.whl"
            artifact.touch()
            args = Namespace(
                absent=False,
                artifact=[artifact],
                build_environment=[],
                config_setting=[],
                distribution=None,
                package_build_variable=[],
                package_config_setting=[],
                lock=root / "pylock.toml",
                manifest=root / "package.json",
                output=root / "environment",
                python=root / "python",
                python_platform="aarch64-apple-darwin",
                requirement=None,
                source_artifact=None,
                source_subdirectory=None,
                source_tree=None,
                uv=root / "uv",
                version=None,
            )

            with patch.object(runner, "_run") as run:
                runner._locked_package(args, {"PATH": "/usr/bin"}, root / "scratch")

            command = run.call_args.args[0]
            self.assertEqual(command[1:4], ["pip", "install", str(artifact)])
            self.assertNotIn(str(args.lock), command)
            self.assertIn("--no-index", command)
            self.assertIn("--no-deps", command)
            self.assertIn("--no-build", command)
            self.assertIn("--offline", command)
            self.assertEqual(
                command[command.index("--python-platform") + 1],
                "aarch64-apple-darwin",
            )

    def test_locked_wheel_candidates_are_selected_offline(self) -> None:
        """Pinned local candidates must replace index access for platform wheels."""
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            candidates = [
                root / "demo-1-cp314-cp314-macosx_13_0_arm64.whl",
                root / "demo-1-py3-none-any.whl",
            ]
            for candidate in candidates:
                candidate.touch()
            args = Namespace(
                absent=False,
                artifact=candidates,
                build_environment=[],
                config_setting=[],
                distribution=None,
                package_build_variable=[],
                package_config_setting=[],
                lock=root / "pylock.toml",
                manifest=root / "package.json",
                output=root / "environment",
                python=root / "python",
                python_platform="aarch64-apple-darwin",
                requirement="demo==1",
                source_artifact=None,
                source_subdirectory=None,
                source_tree=None,
                uv=root / "uv",
                version=None,
            )

            with patch.object(runner, "_run") as run:
                runner._locked_package(args, {"PATH": "/usr/bin"}, root / "scratch")

            command = run.call_args.args[0]
            self.assertIn("demo==1", command)
            self.assertEqual(command.count("--find-links"), 2)
            self.assertIn("--no-index", command)
            self.assertIn("--no-deps", command)
            self.assertIn("--no-build", command)
            self.assertIn("--offline", command)

    def test_absent_locked_package_writes_an_empty_manifest(self) -> None:
        """An excluded marker variant must not invoke uv or consume artifacts."""
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            args = Namespace(
                absent=True,
                artifact=[],
                build_environment=[],
                config_setting=[],
                distribution=None,
                package_build_variable=[],
                package_config_setting=[],
                lock=root / "pylock.toml",
                manifest=root / "package.json",
                output=root / "environment",
                python=root / "python",
                python_platform="aarch64-apple-darwin",
                requirement=None,
                source_artifact=None,
                source_subdirectory=None,
                source_tree=None,
                uv=root / "uv",
                version=None,
            )

            with patch.object(runner, "_run") as run:
                runner._locked_package(args, {"PATH": "/usr/bin"}, root / "scratch")

            run.assert_not_called()
            self.assertEqual(args.manifest.read_text(encoding="utf-8"), "[]\n")

    def test_locked_package_manifests_compose_one_complete_environment(self) -> None:
        """Package granularity must not leak into the runtime import search path."""
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            attrs = root / "attrs"
            typing = root / "typing-extensions"
            (attrs / "namespace").mkdir(parents=True)
            (typing / "namespace").mkdir(parents=True)
            (attrs / "attrs").mkdir()
            (typing / "typing_extensions.py").write_text(
                "VALUE = 'typing-extension'\n", encoding="utf-8"
            )
            (attrs / "attrs" / "__init__.py").write_text(
                "VALUE = 'attrs-package'\n", encoding="utf-8"
            )
            (attrs / "namespace" / "attrs.py").write_text(
                "VALUE = 'attrs'\n", encoding="utf-8"
            )
            (typing / "namespace" / "typing.py").write_text(
                "VALUE = 'typing'\n", encoding="utf-8"
            )
            script = attrs / "bin" / "attrs"
            script.parent.mkdir()
            script.write_text("#!/usr/bin/env python3\n", encoding="utf-8")
            script.chmod(0o755)
            attrs_manifest = root / "attrs.json"
            typing_manifest = root / "typing.json"
            runner._write_package_manifest(attrs, attrs_manifest)
            runner._write_package_manifest(typing, typing_manifest)
            output = root / "overlay"
            manifest = root / "environment.json"

            runner._compose_environment(
                Namespace(
                    output=output,
                    manifest=manifest,
                    package=[
                        ["typing-extensions", typing_manifest, typing],
                        ["attrs", attrs_manifest, attrs],
                    ],
                )
            )

            self.assertEqual(
                (output / "namespace" / "attrs.py").read_text(encoding="utf-8"),
                "VALUE = 'attrs'\n",
            )
            self.assertEqual(
                (output / "namespace" / "typing.py").read_text(encoding="utf-8"),
                "VALUE = 'typing'\n",
            )
            self.assertEqual(
                (output / "attrs" / "__init__.py").read_text(encoding="utf-8"),
                "VALUE = 'attrs-package'\n",
            )
            self.assertEqual(
                (output / "typing_extensions.py").read_text(encoding="utf-8"),
                "VALUE = 'typing-extension'\n",
            )
            provenance = json.loads(manifest.read_text(encoding="utf-8"))
            self.assertEqual(
                provenance["overlay"],
                {
                    "attrs/__init__.py": "attrs",
                    "bin/attrs": "attrs",
                    "namespace/attrs.py": "attrs",
                    "namespace/typing.py": "typing-extensions",
                    "typing_extensions.py": "typing-extensions",
                },
            )
            self.assertTrue(attrs.exists())
            self.assertTrue(typing.exists())

    def test_locked_package_collision_fails_closed(self) -> None:
        """Different distributions may not silently replace the same import file."""
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            left = root / "left"
            right = root / "right"
            left.mkdir()
            right.mkdir()
            (left / "module.py").write_text("OWNER = 'left'\n", encoding="utf-8")
            (right / "module.py").write_text("OWNER = 'right'\n", encoding="utf-8")
            left_manifest = root / "left.json"
            right_manifest = root / "right.json"
            manifest = root / "environment.json"
            runner._write_package_manifest(left, left_manifest)
            runner._write_package_manifest(right, right_manifest)

            with self.assertRaisesRegex(
                RuntimeError,
                "module.py.*left.*right",
            ):
                runner._compose_environment(
                    Namespace(
                        output=root / "overlay",
                        manifest=manifest,
                        package=[
                            ["right", right_manifest, right],
                            ["left", left_manifest, left],
                        ],
                    )
                )

    def test_pep_794_exclusive_import_collision_fails_before_file_shadowing(
        self,
    ) -> None:
        """Distinct files must not hide two distributions claiming one import."""
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            left, left_manifest = _package_with_import_metadata(
                root, "left", "Import-Name: shared", "left.py"
            )
            right, right_manifest = _package_with_import_metadata(
                root, "right", "Import-Name: shared", "right.py"
            )

            with self.assertRaisesRegex(
                RuntimeError,
                "import 'shared'.*left.*right",
            ):
                runner._compose_environment(
                    Namespace(
                        output=root / "overlay",
                        manifest=root / "environment.json",
                        package=[
                            ["right", right_manifest, right],
                            ["left", left_manifest, left],
                        ],
                    )
                )

    def test_pep_794_shared_namespaces_preserve_every_owner(self) -> None:
        """Namespace declarations may overlap and remain queryable provenance."""
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            left, left_manifest = _package_with_import_metadata(
                root, "left", "Import-Namespace: shared", "left.py"
            )
            right, right_manifest = _package_with_import_metadata(
                root, "right", "Import-Namespace: shared", "right.py"
            )
            manifest = root / "environment.json"

            runner._compose_environment(
                Namespace(
                    output=root / "overlay",
                    manifest=manifest,
                    package=[
                        ["right", right_manifest, right],
                        ["left", left_manifest, left],
                    ],
                )
            )

            self.assertEqual(
                json.loads(manifest.read_text(encoding="utf-8"))["imports"],
                {"shared": {"namespace": ["left", "right"]}},
            )

    def test_pep_794_exclusive_and_namespace_ownership_conflict(self) -> None:
        """A namespace may not coexist with another distribution's exclusive name."""
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            exclusive, exclusive_manifest = _package_with_import_metadata(
                root, "exclusive", "Import-Name: shared ; private", "exclusive.py"
            )
            namespace, namespace_manifest = _package_with_import_metadata(
                root, "namespace", "Import-Namespace: shared", "namespace.py"
            )

            with self.assertRaisesRegex(
                RuntimeError,
                "import 'shared'.*exclusive.*namespace",
            ):
                runner._compose_environment(
                    Namespace(
                        output=root / "overlay",
                        manifest=root / "environment.json",
                        package=[
                            ["namespace", namespace_manifest, namespace],
                            ["exclusive", exclusive_manifest, exclusive],
                        ],
                    )
                )

    def test_pep_794_ignores_vendored_distribution_metadata(self) -> None:
        """Only a wheel's top-level core metadata defines its import ownership."""
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            package, _ = _package_with_import_metadata(
                root, "demo", "Import-Name: demo", "demo.py"
            )
            vendored = package / "demo" / "_vendor" / "shared-1.dist-info" / "METADATA"
            vendored.parent.mkdir(parents=True)
            vendored.write_text(
                "Metadata-Version: 2.5\nName: shared\nVersion: 1\n"
                "Import-Name: shared\n",
                encoding="utf-8",
            )
            package_manifest = root / "demo.json"
            runner._write_package_manifest(package, package_manifest)
            shared, shared_manifest = _package_with_import_metadata(
                root, "shared", "Import-Name: shared", "shared.py"
            )
            manifest = root / "environment.json"

            runner._compose_environment(
                Namespace(
                    output=root / "overlay",
                    manifest=manifest,
                    package=[
                        ["demo", package_manifest, package],
                        ["shared", shared_manifest, shared],
                    ],
                )
            )

            self.assertEqual(
                json.loads(manifest.read_text(encoding="utf-8"))["imports"],
                {
                    "demo": {"exclusive": "demo"},
                    "shared": {"exclusive": "shared"},
                },
            )

    def test_pep_794_cross_layer_import_collision_fails_closed(self) -> None:
        """Separate environment layers may not claim one exclusive import."""
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            left, _ = _package_with_import_metadata(
                root, "left", "Import-Name: shared", "left.py"
            )
            right, _ = _package_with_import_metadata(
                root, "right", "Import-Name: shared", "right.py"
            )

            with self.assertRaisesRegex(
                RuntimeError,
                "import 'shared'.*left.*right",
            ):
                runner._write_environment_imports(
                    Namespace(
                        environment=[right, left],
                        output=root / "environment-stack.json",
                    )
                )

    def test_pep_794_cross_layer_namespaces_are_cacheable_provenance(self) -> None:
        """A validated namespace stack writes one deterministic action output."""
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            left, _ = _package_with_import_metadata(
                root, "left", "Import-Namespace: shared", "left.py"
            )
            right, _ = _package_with_import_metadata(
                root, "right", "Import-Namespace: shared", "right.py"
            )
            output = root / "environment-stack.json"

            runner._write_environment_imports(
                Namespace(environment=[right, left], output=output)
            )

            self.assertEqual(
                json.loads(output.read_text(encoding="utf-8")),
                {"shared": {"namespace": ["left", "right"]}},
            )

    def test_pep_794_invalid_or_ambiguous_ownership_fails_closed(self) -> None:
        """Malformed qualifiers and dual ownership never enter provenance."""
        for metadata, expected in (
            ("Import-Name: shared; public", "invalid Import-Name"),
            (
                "Import-Name: shared\nImport-Namespace: shared",
                "both exclusive and namespace",
            ),
        ):
            with (
                self.subTest(metadata=metadata),
                tempfile.TemporaryDirectory() as temporary,
            ):
                root = Path(temporary)
                package, package_manifest = _package_with_import_metadata(
                    root, "demo", metadata, "demo.py"
                )
                with self.assertRaisesRegex(RuntimeError, expected):
                    runner._compose_environment(
                        Namespace(
                            output=root / "overlay",
                            manifest=root / "environment.json",
                            package=[["demo", package_manifest, package]],
                        )
                    )

    def test_locked_package_identical_files_record_every_owner(self) -> None:
        """Byte-identical shared files deduplicate without losing provenance."""
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            left = root / "left"
            right = root / "right"
            for package in (left, right):
                package.mkdir()
                (package / "namespace.py").write_text("VALUE = 1\n", encoding="utf-8")
            left_manifest = root / "left.json"
            right_manifest = root / "right.json"
            manifest = root / "environment.json"
            runner._write_package_manifest(left, left_manifest)
            runner._write_package_manifest(right, right_manifest)

            runner._compose_environment(
                Namespace(
                    output=root / "overlay",
                    manifest=manifest,
                    package=[
                        ["right", right_manifest, right],
                        ["left", left_manifest, left],
                    ],
                )
            )

            provenance = json.loads(manifest.read_text(encoding="utf-8"))
            self.assertEqual(provenance["paths"]["namespace.py"], ["left", "right"])

    def test_locked_package_console_script_collision_matches_uv_precedence(
        self,
    ) -> None:
        """The first canonical package owns a console script, matching uv."""
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            left = root / "left"
            right = root / "right"
            for package, owner in ((left, "left"), (right, "right")):
                script = package / "bin" / "demo"
                script.parent.mkdir(parents=True)
                script.write_text(f"#!/bin/sh\necho {owner}\n", encoding="utf-8")
                script.chmod(0o755)
            left_manifest = root / "left.json"
            right_manifest = root / "right.json"
            runner._write_package_manifest(left, left_manifest)
            runner._write_package_manifest(right, right_manifest)
            output = root / "overlay"
            manifest = root / "environment.json"

            runner._compose_environment(
                Namespace(
                    output=output,
                    manifest=manifest,
                    package=[
                        ["right", right_manifest, right],
                        ["left", left_manifest, left],
                    ],
                )
            )

            self.assertEqual(
                (output / "bin" / "demo").read_text(encoding="utf-8"),
                "#!/bin/sh\necho left\n",
            )
            provenance = json.loads(manifest.read_text(encoding="utf-8"))
            self.assertEqual(provenance["overlay"]["bin/demo"], "left")
            self.assertEqual(provenance["paths"]["bin/demo"], ["left", "right"])

    def test_package_manifest_cannot_escape_the_environment_root(self) -> None:
        """Cached package metadata must not create paths outside its CAS output."""
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            package = root / "package"
            package.mkdir()
            manifest = root / "package.json"
            manifest.write_text(
                json.dumps([["../escaped", "directory"]]), encoding="utf-8"
            )

            with self.assertRaisesRegex(RuntimeError, "not normalized"):
                runner._compose_environment(
                    Namespace(
                        output=root / "overlay",
                        manifest=root / "environment.json",
                        package=[["demo", manifest, package]],
                    )
                )

            self.assertFalse((root / "escaped").exists())

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

    def test_typecheck_uses_native_project_scope_and_import_roots(self) -> None:
        """ty must honor configured sources while resolving cached environments."""
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "source"
            source.mkdir()
            environment = _empty_environment(root / "environment.zip")
            workspace = _empty_environment(root / "workspace.zip")
            output = root / "typecheck.check"
            args = Namespace(
                environment=[environment, workspace],
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
            self.assertEqual(command.count("--extra-search-path"), 2)
            self.assertIn(str(environment.resolve()), command)
            self.assertIn(str(workspace.resolve()), command)
            self.assertEqual(command.count("--output-format"), 1)
            self.assertIn("concise", command)
            self.assertNotIn(".", command)
            self.assertEqual(run.call_args.kwargs["cwd"], source.resolve())

    def test_ruff_uses_native_project_scope(self) -> None:
        """Ruff must select files from pyproject.toml rather than a CLI override."""
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "source"
            source.mkdir()
            args = Namespace(
                environment=[],
                mode="ruff",
                output=root / "lint.check",
                project_root=".",
                python=root / "python",
                source=source,
                ty=None,
                uv=None,
                ruff=root / "ruff",
                vcs=None,
            )

            with patch.object(runner, "_run") as run:
                runner._project(args, {"PATH": "/usr/bin"}, root / "scratch")

            self.assertEqual(
                run.call_args.args[0],
                [
                    str((root / "ruff").resolve()),
                    "check",
                    "--no-cache",
                    "--output-format",
                    "concise",
                ],
            )
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
                environment=[environment],
                config_setting=["editable-mode=strict", "--build-option=--quiet"],
                package_config_setting=["demo:editable-mode=compat"],
                package_build_variable=["demo:DEMO_BUILD=strict"],
                mode="wheel",
                output=root / "wheel",
                project_root=".",
                python=root / "python",
                python_platform="aarch64-apple-darwin",
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
            python_path = action_environment["PYTHONPATH"].split(os.pathsep)
            self.assertEqual(
                Path(python_path[0]).resolve(),
                (root / "scratch" / "target-platform").resolve(),
            )
            self.assertEqual(Path(python_path[1]), materialized_environment)
            self.assertTrue(
                action_environment["PATH"].startswith(
                    str(materialized_environment / "bin")
                )
            )
            self.assertIn("--no-build-isolation", run.call_args.args[0])
            self.assertIn("--offline", run.call_args.args[0])
            self.assertEqual(
                run.call_args.args[0][-4:],
                [
                    "--config-setting=editable-mode=strict",
                    "--config-setting=--build-option=--quiet",
                    "--config-settings-package=demo:editable-mode=compat",
                    ".",
                ],
            )
            config = Path(
                run.call_args.args[0][run.call_args.args[0].index("--config-file") + 1]
            )
            self.assertEqual(
                config.read_text(encoding="utf-8"),
                '[extra-build-variables."demo"]\n"DEMO_BUILD" = "strict"\n',
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
