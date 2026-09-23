# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Verifies build-script diagnostics through the production runner and compiler.

import os
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path


class BuildscriptRunTest(unittest.TestCase):
    def setUp(self) -> None:
        """Create private script outputs with an explicitly selected compiler."""
        temporary = tempfile.TemporaryDirectory(prefix='buildscript-test-')
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name).resolve()
        self.compiler = str(Path(os.environ['RUSTC']).resolve(strict=True))
        self.flags = self.root / 'flags'
        self.source = self.root / 'source'
        self.source.mkdir()
        self.output = self.root / 'out'
        self.cfg = self.root / 'cfg'
        cfg = subprocess.run(
            [self.compiler, '--print=cfg'], capture_output=True, text=True, check=True
        )
        self.cfg.write_text(cfg.stdout)
        version = subprocess.run(
            [self.compiler, '-vV'], capture_output=True, text=True, check=True
        )
        self.host = next(
            line.removeprefix('host: ')
            for line in version.stdout.splitlines()
            if line.startswith('host: ')
        )

    def execute(self, directives: list[str]) -> subprocess.CompletedProcess[str]:
        """Compile and execute a real script through the inherited protocol runner."""
        source = self.source / 'build.rs'
        statements = [f'println!(r##"{line}"##);' for line in directives]
        source.write_text('fn main() {' + ''.join(statements) + '}\n')
        script = self.root / 'build_script'
        subprocess.run(
            [self.compiler, str(source), '-o', str(script)],
            check=True,
            capture_output=True,
            timeout=30,
        )
        command = [
            sys.executable,
            str(Path(__file__).resolve().parents[1] / 'buildscript_run.py'),
            f'--buildscript={script}',
            f'--rustc-cfg={self.cfg}',
            f'--manifest-dir={self.source}',
            f'--create-cwd={self.root / "cwd"}',
            f'--outfile={self.flags}',
        ]
        environment = {
            **os.environ,
            'RUSTC': self.compiler,
            'TARGET': self.host,
            'HOST': self.host,
            'OUT_DIR': str(self.output),
        }
        result = subprocess.run(
            command, env=environment, capture_output=True, text=True, timeout=30
        )
        return result

    def test_invariant_declared_cfg_reaches_the_compiler(self) -> None:
        """Both Cargo prefix forms preserve cfg checking through a real compile."""
        for prefix in ['cargo:', 'cargo::']:
            with self.subTest(prefix=prefix):
                result = self.execute(
                    [
                        f'{prefix}rustc-check-cfg=cfg(selected, values("yes"))',
                        f'{prefix}rustc-cfg=selected="yes"',
                    ]
                )
                self.assertEqual(result.returncode, 0, result.stderr)
                source = self.root / 'lib.rs'
                source.write_text(
                    '#[cfg(selected="yes")] pub fn value() -> u32 { 42 }\n'
                    '#[cfg(not(selected="yes"))] compile_error!("cfg was dropped");\n'
                )
                compiled = subprocess.run(
                    [
                        self.compiler,
                        '--crate-type=lib',
                        '-Dunexpected_cfgs',
                        '--check-cfg=cfg()',
                        *self.flags.read_text().splitlines(),
                        str(source),
                        '-o',
                        str(self.root / 'lib.rlib'),
                    ],
                    capture_output=True,
                    text=True,
                    timeout=30,
                )
                self.assertEqual(compiled.returncode, 0, compiled.stderr)

    def test_invariant_script_error_prevents_flags_publication(self) -> None:
        """A successful process cannot override an explicit Cargo error directive."""
        for prefix in ['cargo:', 'cargo::']:
            with self.subTest(prefix=prefix):
                result = self.execute(
                    [f'{prefix}rustc-cfg=first', f'{prefix}error=missing generator']
                )
                self.assertNotEqual(result.returncode, 0)
                self.assertIn('missing generator', result.stderr)
                self.assertEqual(self.flags.read_text(), '')


if __name__ == '__main__':
    unittest.main()
