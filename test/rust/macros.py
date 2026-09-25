# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Verifies native Cargo macros under an explicitly verified namespace runtime.

import argparse
import json
import os
import re
import shutil
import subprocess
import tempfile
from pathlib import Path


def run(project: Path, *arguments: str) -> subprocess.CompletedProcess[str]:
    """Bound one command with a private artifact cache and explicit diagnostics."""
    environment = {**os.environ, 'BSMR_LOCAL_CACHE_DIR': str(project.parent / 'cache')}
    for key in ['RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS']:
        environment.pop(key, None)
    result = subprocess.run(
        arguments,
        cwd=project,
        env=environment,
        capture_output=True,
        text=True,
        timeout=180,
    )
    return result


def macro(project: Path, expression: str) -> None:
    """Replace the producer while retaining the consumer entrypoint."""
    (project / 'calc/src/lib.rs').write_text(
        'extern crate proc_macro;\n#[proc_macro]\n'
        'pub fn value(_: proc_macro::TokenStream) -> proc_macro::TokenStream {\n'
        + expression
        + '.parse().unwrap()\n}\n'
    )


def build(project: Path, binary: str, expected: str) -> list[str]:
    """Run the new artifact and return its actual compiler executions."""
    result = run(
        project,
        binary,
        'build',
        'app',
        '--sandbox',
        '--console',
        'simple',
        '--show-full-json-output',
    )
    assert result.returncode == 0, result.stderr
    executable = next(iter(json.loads(result.stdout).values()))
    output = run(project, executable)
    assert output.returncode == 0, output.stderr
    assert output.stdout.strip() == expected, output.stdout
    trace = re.search(r'Build ID: ([a-f0-9-]+)', result.stderr)
    assert trace, result.stderr
    log = run(
        project,
        binary,
        'log',
        'what-ran',
        '--trace-id',
        trace[1],
        '--format',
        'json',
        '--filter-category',
        'rustc.*',
        '--no-remote',
    )
    assert log.returncode == 0, log.stderr
    return log.stdout.splitlines()


def initialize(project: Path, binary: str, runtime: Path) -> None:
    """Create a dependency-free Cargo workspace using the binary's embedded rules."""
    files = {
        'Cargo.toml': '[workspace]\nmembers=["app", "calc"]\nresolver="2"\n',
        'rust-toolchain.toml': '[toolchain]\nchannel="1.97.1"\n',
        'app/Cargo.toml': (
            '[package]\nname="app"\nversion="0.1.0"\nedition="2024"\n'
            '[dependencies]\ncalc={path="../calc"}\n'
        ),
        'app/src/main.rs': 'fn main() { println!("{}", calc::value!()); }\n',
        'calc/Cargo.toml': (
            '[package]\nname="calc"\nversion="0.1.0"\nedition="2024"\n[lib]\nproc-macro=true\n'
        ),
        'calc/src/value.txt': '7',
    }
    for path, contents in files.items():
        destination = project / path
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_text(contents)
    macro(project, 'include_str!("value.txt")')
    result = run(
        project, 'rustup', 'run', '1.97.1', 'cargo', 'generate-lockfile', '--offline'
    )
    assert result.returncode == 0, result.stderr
    result = run(project, binary, 'init')
    assert result.returncode == 0, result.stderr
    (project / '.bsmr.local').write_text(
        '[bsmr]\ndefault_allow_cache_upload = true\n'
        f'[sandbox]\nbackend = namespace\nruntime = {runtime}\n'
    )


def qualify(project: Path, binary: str) -> None:
    """Check warm reuse, input invalidation and denied execution on one entrypoint."""
    assert build(project, binary, '7'), 'cold build must execute native compilers'
    assert build(project, binary, '7') == [], (
        'unchanged consumers must reuse their actions'
    )
    result = run(project, binary, 'build', 'app', '--console', 'simple')
    assert result.returncode != 0, 'host execution must not reuse isolated analysis'
    assert 'verified declared-input executor' in result.stderr, result.stderr
    assert build(project, binary, '7') == [], (
        'isolated execution must reuse its own results'
    )
    clone = project.with_name('clone')
    shutil.copytree(project, clone, ignore=shutil.ignore_patterns('bsmr-out', 'target'))
    try:
        reused = build(clone, binary, '7')
        assert not any('"executor":"Local"' in action for action in reused), reused
        assert any('"executor":"Cache"' in action for action in reused), reused
    finally:
        result = run(clone, binary, 'kill')
        assert result.returncode == 0, result.stderr
    (project / 'calc/src/value.txt').write_text('9')
    assert build(project, binary, '9'), 'a macro input change must rebuild the consumer'
    grammar = project / 'app/grammar.txt'
    grammar.write_text('17')
    macro(project, '''{
        let root = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
        assert!(root.is_absolute());
        assert_eq!(root.join("Cargo.toml"),
            std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_PATH").unwrap()));
        std::fs::read_to_string(root.join("grammar.txt")).unwrap()
    }''')
    reference = run(
        project, 'rustup', 'run', '1.97.1', 'cargo', 'run', '--locked', '--offline', '-p', 'app'
    )
    assert reference.returncode == 0, reference.stderr
    assert reference.stdout.strip() == '17', reference.stdout
    assert build(project, binary, '17'), 'macros must read their caller package files'
    grammar.write_text('19')
    assert build(project, binary, '19'), 'a caller file edit must invalidate expansion'
    assert build(project, binary, '19') == [], 'unchanged caller files must reuse expansion'
    outside = project.parent / 'outside'
    macro(
        project,
        f'std::fs::read_to_string({json.dumps(str(outside))}).expect("undeclared read")',
    )
    for value in ['11', '13']:
        outside.write_text(value)
        result = run(
            project, binary, 'build', 'app', '--sandbox', '--console', 'simple'
        )
        assert result.returncode != 0, 'an external read must fail on every invocation'
        assert 'undeclared read' in result.stderr, result.stderr
    print(
        'ok  Cargo macros: native execution, warm reuse, input edits, '
        'host refusal, denied external reads'
    )


def main() -> None:
    """Run qualification with the caller's binary and independently pinned runtime."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('binary', type=Path)
    parser.add_argument('runtime', type=Path)
    args = parser.parse_args()
    binary = str(args.binary.resolve(strict=True))
    runtime = args.runtime.resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix='bsmr-macros-') as temporary:
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
