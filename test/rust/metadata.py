# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Verifies Cargo metadata visibility and generated paths across cached native actions.

import argparse
import shutil
import tempfile
from pathlib import Path

from macros import build, initialize, run

PRODUCER = """
fn main() {
    let value = std::fs::read_to_string("value.txt").unwrap();
    let out = std::env::var("OUT_DIR").unwrap();
    std::fs::write(format!("{out}/value.txt"), &value).unwrap();
    println!("cargo::metadata=include={out}/value.txt");
    println!("cargo::metadata=source={}/value.txt", std::env::var("CARGO_MANIFEST_DIR").unwrap());
    println!("cargo:old-key=legacy");
    println!("cargo:error=legacy metadata");
    println!("cargo::metadata=a-key=first");
    println!("cargo::metadata=A_KEY=second");
    println!("cargo::metadata=a-key=last");
    println!("cargo::rerun-if-changed=value.txt");
}
"""

CONSUMER = """
fn main() {
    assert_eq!(std::env::var("DEP_NATIVE_API_OLD_KEY").unwrap(), "legacy");
    assert_eq!(std::env::var("DEP_NATIVE_API_ERROR").unwrap(), "legacy metadata");
    assert_eq!(std::env::var("DEP_NATIVE_API_A_KEY").unwrap(), "last");
    let value = std::fs::read_to_string(std::env::var("DEP_NATIVE_API_INCLUDE").unwrap()).unwrap();
    let source = std::fs::read_to_string(std::env::var("DEP_NATIVE_API_SOURCE").unwrap()).unwrap();
    assert_eq!(value, source);
    println!("cargo::metadata=value={value}");
    println!("cargo::rustc-env=COMPUTED={value}");
}
"""


def qualify(project: Path, binary: str) -> None:
    """Compare direct dependency metadata with Cargo, including cache restoration."""
    files = {
        'Cargo.toml': (
            '[workspace]\nmembers=["app","calc","native","middle","guard"]\nresolver="2"\n'
        ),
        'native/Cargo.toml': (
            '[package]\nname="native_sys"\nversion="0.1.0"\nedition="2024"\nlinks="native-api"\n'
        ),
        'native/build.rs': PRODUCER,
        'native/src/lib.rs': '',
        'native/value.txt': '11',
        'middle/Cargo.toml': (
            '[package]\nname="middle"\nversion="0.1.0"\nedition="2024"\nlinks="middle-api"\n'
            '[dependencies]\nnative_sys={path="../native"}\nguard={path="../guard"}\n'
        ),
        'middle/build.rs': CONSUMER,
        'middle/src/lib.rs': 'pub fn value() -> &\'static str { env!("COMPUTED") }\n',
        'guard/Cargo.toml': (
            '[package]\nname="guard"\nversion="0.1.0"\nedition="2024"\nlinks="unique-runtime"\n'
        ),
        'guard/build.rs': 'fn main() { println!("cargo:rerun-if-changed=build.rs"); }\n',
        'guard/src/lib.rs': '',
        'app/build.rs': """fn main() {
            assert!(std::env::var("DEP_NATIVE_API_INCLUDE").is_err());
            let value = std::env::var("DEP_MIDDLE_API_VALUE").unwrap();
            println!("cargo::rustc-env=APP_VALUE={value}");
        }""",
        'app/src/main.rs': (
            'fn main() { println!("{}:{}:{}", '
            'calc::value!(), middle::value(), env!("APP_VALUE")); }\n'
        ),
    }
    for path, contents in files.items():
        destination = project / path
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(contents)
    manifest = project / 'app/Cargo.toml'
    manifest.write_text(manifest.read_text() + 'middle={path="../middle"}\n')
    locked = run(
        project, 'rustup', 'run', '1.97.1', 'cargo', 'generate-lockfile', '--offline'
    )
    assert locked.returncode == 0, locked.stderr
    reference = run(
        project,
        'rustup',
        'run',
        '1.97.1',
        'cargo',
        'run',
        '--locked',
        '--offline',
        '-p',
        'app',
    )
    assert reference.returncode == 0, reference.stderr
    assert reference.stdout.strip() == '7:11:11', reference.stdout
    assert build(project, binary, '7:11:11')
    assert build(project, binary, '7:11:11') == []
    clone = project.with_name('clone')
    shutil.copytree(project, clone, ignore=shutil.ignore_patterns('bsmr-out', 'target'))
    try:
        actions = build(clone, binary, '7:11:11')
        assert not any('"executor":"Local"' in action for action in actions), actions
        assert any('"executor":"Cache"' in action for action in actions), actions
    finally:
        result = run(clone, binary, 'kill')
        assert result.returncode == 0, result.stderr
    (project / 'native/value.txt').write_text('13')
    assert build(project, binary, '7:13:13')
    print(
        'ok  Cargo metadata: direct visibility, ordering, generated paths, '
        'empty links, cache restoration, edits'
    )


def main() -> None:
    """Qualify an explicit engine and namespace runtime without mutating either."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('binary', type=Path)
    parser.add_argument('runtime', type=Path)
    args = parser.parse_args()
    binary = str(args.binary.resolve(strict=True))
    runtime = args.runtime.resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix='bsmr-metadata-') as temporary:
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
