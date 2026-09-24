# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Compares a caller-selected linker with Cargo under a pinned execution runtime.

import argparse
import json
from pathlib import Path
import tempfile

from macros import build, run


def configure(project: Path, driver: str, argument: str) -> None:
    """Preserve the exact driver and argument selected by the caller."""
    (project / '.cargo/config.toml').write_text(
        '[target.\'cfg(target_os = "linux")\']\n'
        f'linker={json.dumps(driver)}\n'
        f'rustflags={json.dumps(["-C", "link-arg=" + argument])}\n'
    )


def qualify(project: Path, binary: str, runtime: Path, driver: str, argument: str) -> None:
    """Check the actual executable, build-script contract and warm cache isolation."""
    files = {
        'Cargo.toml': '[workspace]\nmembers=["app"]\nresolver="2"\n',
        'rust-toolchain.toml': '[toolchain]\nchannel="1.97.1"\n',
        'app/Cargo.toml': '[package]\nname="app"\nversion="0.1.0"\nedition="2024"\n',
        'app/src/main.rs': 'fn main() { println!("17"); }\n',
        'app/build.rs': f'fn main() {{ assert_eq!(std::env::var("RUSTC_LINKER").unwrap(), {json.dumps(driver)}); }}\n',
    }
    for name, text in files.items():
        path = project / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_text(text)
    (project / '.cargo').mkdir()
    configure(project, driver, argument)
    for command in [('cargo', 'generate-lockfile', '--offline'), ('cargo', 'run', '--locked', '--offline', '-p', 'app')]:
        result = run(project, *command)
        assert result.returncode == 0, result.stderr
    assert result.stdout.strip() == '17', result.stdout
    result = run(project, binary, 'init')
    assert result.returncode == 0, result.stderr
    (project / '.bsmr.local').write_text(
        '[bsmr]\ndefault_allow_cache_upload=true\n'
        f'[sandbox]\nbackend=namespace\nruntime={runtime}\n'
    )
    assert build(project, binary, '17'), 'cold build must run the configured linker'
    assert build(project, binary, '17') == [], 'unchanged linking must reuse actions'
    configure(project, driver, '-fuse-ld=unavailable-bsmr-test-linker')
    result = run(project, binary, 'build', 'app', '--sandbox', '--console', 'simple')
    assert result.returncode != 0, 'a changed linker must not reuse the previous binary'
    assert 'unavailable-bsmr-test-linker' in result.stderr, result.stderr
    configure(project, 'unavailable-bsmr-test-driver', argument)
    result = run(project, binary, 'build', 'app', '--sandbox', '--console', 'simple')
    assert result.returncode != 0, 'a missing driver must fail without substitution'
    assert 'unavailable-bsmr-test-driver' in result.stderr, result.stderr
    configure(project, driver, argument)
    restored = build(project, binary, '17')
    assert not any('"executor":"Local"' in action for action in restored), restored
    result = run(project, binary, 'build', 'app', '--console', 'simple')
    assert result.returncode != 0 and 'verified declared-input executor' in result.stderr
    print('ok: configured driver, link argument, script environment and cache isolation')


def main() -> None:
    """Require the engine, pinned runtime, driver and link argument explicitly."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('binary', type=Path)
    parser.add_argument('runtime', type=Path)
    parser.add_argument('driver')
    parser.add_argument('argument')
    args = parser.parse_args()
    binary = str(args.binary.resolve(strict=True))
    with tempfile.TemporaryDirectory(prefix='bsmr-linker-') as temporary:
        project = Path(temporary) / 'project'
        project.mkdir()
        try:
            qualify(project, binary, args.runtime.resolve(strict=True), args.driver, args.argument)
        finally:
            if (project / '.bsmr').exists():
                stopped = run(project, binary, 'kill')
                assert stopped.returncode == 0, stopped.stderr


if __name__ == '__main__':
    main()
