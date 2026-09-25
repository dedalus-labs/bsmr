# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Verifies native Cargo tests against Cargo's binary, feature and source contracts.

import argparse
import json
from pathlib import Path
import tempfile

from macros import run


INTEGRATION = '''
#[test]
fn declared_inputs() {
    assert_eq!(value::number(), 9);
    assert_eq!(std::fs::read_to_string("value.txt").unwrap(), "fixture");
    assert_eq!(std::fs::read_to_string("../value/fixture.txt").unwrap(), "dependency");
    let manifest = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(manifest.is_absolute());
    assert!(manifest.join("tests/cli.rs").ends_with(file!()), "{}", file!());
    assert_eq!(std::fs::read_to_string(manifest.join("value.txt")).unwrap(), "fixture");
    let binary = std::path::Path::new(env!("CARGO_BIN_EXE_app"));
    assert!(binary.is_absolute());
    let output = std::process::Command::new(binary).current_dir("/").output().unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap().trim(), "9");
}
'''

CUSTOM = '''
fn main() {
    assert_eq!(value::number(), 9);
    assert!(cfg!(test));
    println!("CUSTOM_HARNESS_RAN");
}
'''


def initialize(project: Path, binary: str, runtime: Path | None, package: str) -> None:
    """Create one normal binary and two test entrypoints sharing a dev feature."""
    files = {
        'Cargo.toml': '[workspace]\nmembers=["app", "value"]\nresolver="2"\n',
        'rust-toolchain.toml': '[toolchain]\nchannel="1.97.1"\n',
        'app/Cargo.toml': '''[package]
name="app"
version="0.1.0"
edition="2024"
[[test]]
name="custom"
harness=false
[dependencies]
value={path="../value"}
[dev-dependencies]
value={path="../value",features=["testing"]}
''',
        'app/src/main.rs': 'fn main() { println!("{}", value::number()); }\n',
        'app/tests/cli.rs': INTEGRATION,
        'app/tests/custom.rs': CUSTOM,
        'app/value.txt': 'fixture',
        'value/Cargo.toml': '[package]\nname="value"\nversion="0.1.0"\nedition="2024"\n[features]\ntesting=[]\n[lints.rust]\nunexpected_cfgs="deny"\n',
        'value/src/lib.rs': '#[cfg(docsrs)] pub fn documentation() {}\n#[cfg(test)] mod tests {}\npub fn number()->u32 { if cfg!(feature="testing") {9} else {7} }\n',
        'value/fixture.txt': 'dependency',
    }
    if runtime is not None:
        files['app/build.rs'] = 'fn main() { std::fs::write("generated.txt", "generated").unwrap(); }\n'
        files['app/generated.txt'] = 'stale'
        files['data/value.txt'] = 'sibling'
        files['value/src/lib.rs'] += 'const _: &str = include_str!("../../data/value.txt");\n'
        if package:
            files['value/build.rs'] = 'fn main() {}\n'
        files['app/tests/cli.rs'] = files['app/tests/cli.rs'].replace(
            'fn declared_inputs() {',
            'fn declared_inputs() { assert_eq!(std::fs::read_to_string("generated.txt").unwrap(), "generated");',
        )
    if not package:
        manifest = files.pop('app/Cargo.toml').replace('path="../value"', 'path="value"')
        files['Cargo.toml'] = manifest + '\n[workspace]\nmembers=["value"]\nresolver="2"\n'
        files = {name.removeprefix('app/'): content for name, content in files.items()}
        files['tests/cli.rs'] = files['tests/cli.rs'].replace('../value/', 'value/')
    for name, content in files.items():
        destination = project / name
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(content)
    result = run(project, 'cargo', 'generate-lockfile', '--offline')
    assert result.returncode == 0, result.stderr
    result = run(project, binary, 'init')
    assert result.returncode == 0, result.stderr
    if runtime is not None:
        (project / '.bsmr.local').write_text(
            f'[sandbox]\nbackend=namespace\nruntime={runtime}\n'
        )


def ordinary(project: Path, binary: str, execution: tuple[str, ...], target: str) -> None:
    """The normal binary must not inherit the test graph's dependency features."""
    result = run(project, binary, 'build', target, *execution, '--show-full-json-output')
    assert result.returncode == 0, result.stderr
    executable = next(iter(json.loads(result.stdout).values()))
    output = run(project, executable)
    assert output.returncode == 0, output.stderr
    assert output.stdout.strip() == '7', output.stdout


def qualify(project: Path, binary: str, execution: tuple[str, ...], package: str) -> None:
    """Run the same tests with Cargo and BSMR, including observable failures."""
    target = package if package else '//:app'
    prefix = f'{package}:' if package else '//:'
    reference = run(project, 'cargo', 'test', '--locked', '--offline', '-p', 'app')
    assert reference.returncode == 0, reference.stderr
    assert 'CUSTOM_HARNESS_RAN' in reference.stdout, reference.stdout
    if (project / package / 'build.rs').exists():
        (project / package / 'generated.txt').write_text('stale')
    ordinary(project, binary, execution, target)
    for selected in [prefix + '__bsmr_test_test_cli', prefix + '__bsmr_test_test_custom', target]:
        result = run(project, binary, 'test', selected, *execution, '--console', 'simple')
        assert result.returncode == 0, result.stderr
        assert 'NO TESTS RAN' not in result.stderr, result.stderr
    ordinary(project, binary, execution, target)
    fixture = project / 'value/fixture.txt'
    fixture.write_text('changed')
    changed = run(project, binary, 'test', prefix + '__bsmr_test_test_cli', *execution, '--console', 'simple')
    assert changed.returncode != 0, 'dependency fixture edits must invalidate the test view'
    assert '"changed"' in changed.stderr, changed.stderr
    fixture.write_text('dependency')
    (project / package / 'tests/custom.rs').write_text('fn main() { panic!("CUSTOM_FAILURE"); }')
    result = run(project, binary, 'test', target, *execution, '--console', 'simple')
    assert result.returncode != 0, 'package selection must execute its custom harness'
    assert 'CUSTOM_FAILURE' in result.stderr, result.stderr
    if (project / package / 'build.rs').exists():
        assert (project / package / 'generated.txt').read_text() == 'stale'
    print('ok: Cargo test features, native binaries, declared files, custom harnesses and failures')


def main() -> None:
    """Use local execution or the caller's explicitly selected namespace runtime."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('binary', type=Path)
    parser.add_argument('runtime', type=Path, nargs='?')
    args = parser.parse_args()
    binary = str(args.binary.resolve(strict=True))
    with tempfile.TemporaryDirectory(prefix='bsmr-tests-') as temporary:
        for package in ['app', '']:
            project = Path(temporary) / ('member' if package else 'root')
            project.mkdir()
            try:
                initialize(project, binary, args.runtime.resolve(strict=True) if args.runtime else None, package)
                qualify(project, binary, ('--sandbox',) if args.runtime else (), package)
            finally:
                if (project / '.bsmr').exists():
                    stopped = run(project, binary, 'kill')
                    assert stopped.returncode == 0, stopped.stderr


if __name__ == '__main__':
    main()
