# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Verifies compiler input checks before successful artifact or diagnostic publication.

import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


class RustcActionTest(unittest.TestCase):
    def setUp(self) -> None:
        """Require an explicit real compiler and private files for each invocation."""
        self.compiler = str(Path(os.environ["RUSTC"]).resolve(strict=True))
        temporary = tempfile.TemporaryDirectory(prefix="rustc-action-test-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.source = self.root / "lib.rs"
        self.data = self.root / "data.txt"
        self.data.write_text("input")
        self.dep_info = self.root / "deps.d"
        self.status = self.root / "status.json"
        self.output = self.root / "lib.rmeta"

    def compile(
        self,
        source: str,
        *,
        allow_data: bool = False,
        failure_filter: bool = True,
        emit_dep_info: bool = True,
    ) -> subprocess.CompletedProcess[str]:
        """Run the production wrapper with real rustc and explicitly declared source inputs."""
        for output in [self.dep_info, self.status, self.output]:
            output.unlink(missing_ok=True)
        self.source.write_text(source)
        command = [
            sys.executable,
            str(Path(__file__).resolve().parents[1] / "rustc_action.py"),
            "--dep-info",
            str(self.dep_info),
            "--allowed-input",
            str(self.source),
        ]
        if allow_data:
            command.extend(["--allowed-input", str(self.data)])
        if failure_filter:
            command.extend(
                [
                    "--failure-filter",
                    str(self.status),
                    "--required-output",
                    "lib.rmeta",
                    str(self.output),
                ]
            )
        emits = f"dep-info={self.dep_info},metadata" if emit_dep_info else "metadata"
        command.extend(
            [
                self.compiler,
                "--rustc-action-separator",
                "--crate-type=lib",
                "--error-format=json",
                f"--emit={emits}",
                str(self.source),
                "-o",
                str(self.output),
            ]
        )
        result = subprocess.run(
            command,
            cwd=self.root,
            capture_output=True,
            text=True,
            timeout=30,
            env={**os.environ, "HOME": str(self.root / "undeclared-home")},
        )
        return result

    def include_source(self, *, type_error: bool) -> str:
        """Read the fixture file before either a successful compile or a type error."""
        statement = 'let _: u32 = "type error";' if type_error else ""
        result = (
            f"const DATA: &str = include_str!({json.dumps(str(self.data))});\n"
            f"pub fn value() {{ {statement} }}\n"
        )
        return result

    def test_artifact_success_requires_declared_source_reads(self) -> None:
        result = self.compile(
            self.include_source(type_error=False), failure_filter=False
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("undeclared Rust source input:", result.stderr)

    def test_diagnostic_success_requires_declared_source_reads(self) -> None:
        result = self.compile(self.include_source(type_error=True))
        self.assertIn(str(self.data), self.dep_info.read_text())
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("undeclared Rust source input:", result.stderr)

    def test_diagnostic_success_requires_declared_environment_reads(self) -> None:
        result = self.compile(
            'const HOME: &str = env!("HOME");\npub fn value() { let _: u32 = "error"; }\n'
        )
        self.assertIn("# env-dep:HOME=", self.dep_info.read_text())
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("undeclared Rust environment input: HOME", result.stderr)

    def test_declared_inputs_preserve_compiler_and_diagnostic_success(self) -> None:
        for type_error in [False, True]:
            with self.subTest(type_error=type_error):
                result = self.compile(
                    self.include_source(type_error=type_error), allow_data=True
                )
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(
                    json.loads(self.status.read_text())["status"], int(type_error)
                )
                self.assertTrue(self.output.is_file())

    def test_diagnostic_success_requires_a_dependency_report(self) -> None:
        result = self.compile(
            'pub fn value() { let _: u32 = "error"; }', emit_dep_info=False
        )
        self.assertFalse(self.dep_info.exists())
        self.assertNotEqual(result.returncode, 0)
        self.assertIn(str(self.dep_info), result.stderr)


if __name__ == "__main__":
    unittest.main()
