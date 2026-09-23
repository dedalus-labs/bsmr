# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Verifies build-script diagnostics through the production runner and compiler.

import os
import shutil
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
        self.metadata = self.root / 'metadata.json'
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

    def execute(
        self,
        directives: list[str],
        assertions: str = '',
        dependencies: tuple[str, ...] = (),
    ) -> subprocess.CompletedProcess[str]:
        """Compile and execute a real script through the inherited protocol runner."""
        source = self.source / 'build.rs'
        statements = [f'println!(r##"{line}"##);' for line in directives]
        source.write_text('fn main() {' + assertions + ''.join(statements) + '}\n')
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
            f'--metadata-out={self.metadata}',
            *dependencies,
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
        result = self.execute(
            ['cargo::rustc-cfg=first', 'cargo::error=missing generator']
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('missing generator', result.stderr)
        self.assertEqual(self.flags.read_text(), '')

    def test_invariant_malformed_metadata_cannot_publish_flags(self) -> None:
        """Incomplete metadata invalidates the complete script output."""
        result = self.execute(
            ['cargo::rustc-cfg=first', 'cargo::metadata=missing-separator']
        )
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('invalid build-script metadata', result.stderr)
        self.assertEqual(self.flags.read_text(), '')
        self.assertFalse(self.metadata.exists())

    def test_invariant_cfg_environment_preserves_custom_values(self) -> None:
        """Build scripts receive custom cfgs with Cargo's boolean and value semantics."""
        cfg = subprocess.run(
            [
                self.compiler,
                '--print=cfg',
                '--cfg',
                'input_cfg',
                '--cfg',
                'label="a=b"',
            ],
            capture_output=True,
            text=True,
            check=True,
        )
        self.cfg.write_text(cfg.stdout)
        result = self.execute(
            [],
            """
            assert_eq!(std::env::var("CARGO_CFG_INPUT_CFG").unwrap(), "");
            assert_eq!(std::env::var("CARGO_CFG_LABEL").unwrap(), "a=b");
        """,
        )
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_invariant_unmodeled_directives_cannot_publish_success(self) -> None:
        """A requested compiler effect must be represented or rejected."""
        result = self.execute(['cargo::rustc-link-arg=-unknown-linker-option'])
        self.assertNotEqual(result.returncode, 0)
        self.assertIn('unsupported build-script directive', result.stderr)

    def test_invariant_metadata_preserves_order_and_relocated_files(self) -> None:
        """Dependent scripts receive ordered metadata bound to the restored outputs."""
        result = self.execute(
            [
                'cargo:old-key=legacy',
                'cargo:error=legacy metadata',
                'cargo::metadata=a-key=first',
                'cargo::metadata=A_KEY=second',
                'cargo::metadata=a-key=last',
            ],
            """
            let out = std::env::var("OUT_DIR").unwrap();
            std::fs::write(format!("{out}/value"), "17").unwrap();
            println!("cargo::metadata=include={out}/value");
            println!("cargo::metadata=source={}", std::env::var("CARGO_MANIFEST_DIR").unwrap());
            """,
        )
        self.assertEqual(result.returncode, 0, result.stderr)
        metadata = self.root / 'producer.json'
        self.metadata.rename(metadata)
        output = self.root / 'restored output'
        cwd = self.root / 'restored source'
        shutil.move(self.output, output)
        shutil.move(self.root / 'cwd', cwd)
        result = self.execute(
            [],
            """
            assert_eq!(std::env::var("DEP_NATIVE_API_OLD_KEY").unwrap(), "legacy");
            assert_eq!(std::env::var("DEP_NATIVE_API_A_KEY").unwrap(), "last");
            assert_eq!(std::env::var("DEP_NATIVE_API_ERROR").unwrap(), "legacy metadata");
            let include = std::env::var("DEP_NATIVE_API_INCLUDE").unwrap();
            assert_eq!(std::fs::read_to_string(include).unwrap(), "17");
            assert!(std::path::Path::new(&std::env::var("DEP_NATIVE_API_SOURCE").unwrap()).is_dir());
        """,
            (
                '--metadata-dependency',
                'DEP_native-api',
                str(metadata),
                str(output),
                str(cwd),
            ),
        )
        self.assertEqual(result.returncode, 0, result.stderr)


if __name__ == '__main__':
    unittest.main()
