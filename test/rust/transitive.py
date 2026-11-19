# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Verifies that isolated compiler actions receive complete transitive dependency files.

import argparse
import shutil
import tempfile
from pathlib import Path

from macros import build, initialize, run


def qualify(project: Path, binary: str) -> None:
    """Retain transitive source files and generated macro inputs across cache restores."""
    files = {
        'Cargo.toml': '[workspace]\nmembers=["app","middle","leaf"]\nresolver="2"\n',
        'app/Cargo.toml': (
            '[package]\nname="app"\nversion="0.1.0"\nedition="2024"\n'
            '[dependencies]\nmiddle={path="../middle"}\n'
        ),
        'app/src/main.rs': 'fn main() { println!("{}", middle::value()); }\n',
        'middle/Cargo.toml': (
            '[package]\nname="middle"\nversion="0.1.0"\nedition="2024"\n'
            '[dependencies]\nleaf={path="../leaf"}\n'
        ),
        'middle/src/lib.rs': 'pub fn value() -> u32 { leaf::value() + leaf::offset!() }\n',
        'leaf/Cargo.toml': '[package]\nname="leaf"\nversion="0.1.0"\nedition="2024"\n',
        'leaf/offset.txt': '0',
        'leaf/build.rs': '''fn main() {
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    std::fs::write(out.join("offset.rs"), std::fs::read("offset.txt").unwrap()).unwrap();
    let code = format!(
        "#[macro_export] macro_rules! offset {{ () => {{ include!({:?}) }} }}",
        out.join("offset.rs"),
    );
    std::fs::write(out.join("macros.rs"), code).unwrap();
}
''',
        'leaf/src/lib.rs': (
            'include!(concat!(env!("OUT_DIR"), "/macros.rs"));\n'
            'pub fn value() -> u32 { 7 }\n'
        ),
    }
    for path, contents in files.items():
        destination = project / path
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(contents)
    locked = run(
        project, 'rustup', 'run', '1.97.1', 'cargo', 'generate-lockfile', '--offline'
    )
    assert locked.returncode == 0, locked.stderr
    assert build(project, binary, '7')
    assert build(project, binary, '7') == []
    clone = project.with_name('clone')
    shutil.copytree(project, clone, ignore=shutil.ignore_patterns('bsmr-out', 'target'))
    try:
        actions = build(clone, binary, '7')
        assert not any('"executor":"Local"' in action for action in actions), actions
        assert any('"executor":"Cache"' in action for action in actions), actions
    finally:
        result = run(clone, binary, 'kill')
        assert result.returncode == 0, result.stderr
    (project / 'leaf/src/lib.rs').write_text(files['leaf/src/lib.rs'].replace('{ 7 }', '{ 9 }'))
    assert build(project, binary, '9')
    (project / 'leaf/offset.txt').write_text('2')
    assert build(project, binary, '11'), 'generated macro input edits must rebuild consumers'
    print(
        'ok  transitive dependencies: isolation, warm reuse, restored inputs, edited leaf'
    )


def main() -> None:
    """Use the supplied engine and verified runtime for the actual compiler path."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('binary', type=Path)
    parser.add_argument('runtime', type=Path)
    args = parser.parse_args()
    binary = str(args.binary.resolve(strict=True))
    runtime = args.runtime.resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix='bsmr-transitive-') as temporary:
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
