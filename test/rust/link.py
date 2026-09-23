# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Verifies that build-script linker arguments affect only Cargo's linking targets.

import argparse
from pathlib import Path
import tempfile

from macros import build, initialize, run


def qualify(project: Path, binary: str) -> None:
    """Compare library, macro and binary linker-argument behavior with Cargo."""
    invalid = 'fn main() { println!("cargo::rustc-link-arg=-Wl,--bsmr-invalid"); }\n'
    (project / 'Cargo.toml').write_text(
        '[workspace]\nmembers=["app","calc","dep"]\nresolver="2"\n'
    )
    manifest = project / 'app/Cargo.toml'
    manifest.write_text(manifest.read_text() + 'dep={path="../dep"}\n')
    (project / 'dep/src').mkdir(parents=True)
    (project / 'dep/Cargo.toml').write_text(
        '[package]\nname="dep"\nversion="0.1.0"\nedition="2024"\n'
    )
    (project / 'dep/src/lib.rs').write_text('pub fn value() -> u32 { 0 }\n')
    (project / 'dep/build.rs').write_text(invalid)
    (project / 'app/src/main.rs').write_text(
        'fn main() { println!("{}", calc::value!() + dep::value()); }\n'
    )
    locked = run(
        project, 'rustup', 'run', '1.97.1', 'cargo', 'generate-lockfile', '--offline'
    )
    assert locked.returncode == 0, locked.stderr
    reference = run(
        project, 'rustup', 'run', '1.97.1', 'cargo', 'run', '--locked', '-p', 'app'
    )
    assert reference.returncode == 0, reference.stderr
    assert reference.stdout.strip() == '7', reference.stdout
    assert build(project, binary, '7')
    for package in ['calc', 'app']:
        script = project / package / 'build.rs'
        script.write_text(invalid)
        for command in [
            ['rustup', 'run', '1.97.1', 'cargo', 'build', '--locked', '-p', 'app'],
            [binary, 'build', 'app', '--sandbox', '--console', 'simple'],
        ]:
            failed = run(project, *command)
            assert failed.returncode != 0, failed.stdout
            assert '--bsmr-invalid' in failed.stderr, failed.stderr
        script.write_text('fn main() { println!("cargo::rustc-link-arg=-pthread"); }\n')
        assert build(project, binary, '7')
    assert build(project, binary, '7') == []
    print(
        'ok  linker arguments: Cargo parity, libraries, macros, binaries, edits, reuse'
    )


def main() -> None:
    """Run the actual compiler and GNU linker under an explicitly verified runtime."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('binary', type=Path)
    parser.add_argument('runtime', type=Path)
    args = parser.parse_args()
    binary = str(args.binary.resolve(strict=True))
    runtime = args.runtime.resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix='bsmr-link-') as temporary:
        project = Path(temporary) / 'project'
        project.mkdir()
        try:
            initialize(project, binary, runtime)
            qualify(project, binary)
        finally:
            if (project / '.bsmr').exists():
                result = run(project, binary, 'kill')
                assert result.returncode == 0, result.stderr


if __name__ == '__main__':
    main()
