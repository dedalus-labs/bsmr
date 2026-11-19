# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Verifies that inferred compiler downloads carry complete content identities.

import argparse
import json
from pathlib import Path
import tempfile

from macros import run


def main() -> None:
    """Inspect the real inferred graph without contacting the distribution server."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('binary', type=Path)
    args = parser.parse_args()
    binary = str(args.binary.resolve(strict=True))
    catalog = (
        Path(__file__).resolve().parents[2]
        / 'app/bsmr_common/src/rust_graph/releases.json'
    )
    with tempfile.TemporaryDirectory(prefix='bsmr-catalog-') as temporary:
        project = Path(temporary) / 'project'
        project.mkdir()
        (project / 'Cargo.toml').write_text(
            '[package]\nname="probe"\nversion="0.1.0"\nedition="2024"\n'
        )
        (project / 'rust-toolchain.toml').write_text('[toolchain]\nchannel="1.97.1"\n')
        (project / 'src').mkdir()
        (project / 'src/lib.rs').write_text('pub fn value() -> u32 { 7 }\n')
        try:
            locked = run(
                project,
                'rustup',
                'run',
                '1.97.1',
                'cargo',
                'generate-lockfile',
                '--offline',
            )
            assert locked.returncode == 0, locked.stderr
            compiler = run(project, 'rustup', 'run', '1.97.1', 'rustc', '-vV')
            assert compiler.returncode == 0, compiler.stderr
            host = next(
                line.removeprefix('host: ')
                for line in compiler.stdout.splitlines()
                if line.startswith('host: ')
            )
            expected = json.loads(catalog.read_text())['1.97.1'][host]
            initialized = run(project, binary, 'init')
            assert initialized.returncode == 0, initialized.stderr
            labels = [f'root//:__bsmr_{name}' for name in expected]
            result = run(project, binary, 'targets', *labels, '--json')
            assert result.returncode == 0, result.stderr
            targets = json.loads(result.stdout)
            assert len(targets) == len(expected), targets
            for target in targets:
                archive = expected[target['name'].removeprefix('__bsmr_')]
                assert target['urls'] == [archive['url']], target
                assert target['sha256'] == archive['sha256'], target
                assert target.get('size_bytes') == archive['size_bytes'] > 0, target
            print(
                'ok  compiler catalog: inferred downloads preserve URL, digest and size'
            )
        finally:
            if (project / '.bsmr').exists():
                stopped = run(project, binary, 'kill')
                assert stopped.returncode == 0, stopped.stderr


if __name__ == '__main__':
    main()
