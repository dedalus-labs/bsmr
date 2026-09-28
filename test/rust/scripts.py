# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Verifies Cargo build-script outputs through the binary's native rules.

import argparse
from pathlib import Path
import shutil
import tempfile

from macros import build, initialize, run


SCRIPT = """
fn main() {
    assert_eq!(std::env::var("CARGO_FEATURE_FAST").unwrap(), "1");
    assert_eq!(std::env::var("CARGO_CFG_INPUT_CFG").unwrap(), "");
    assert_eq!(std::env::var("CARGO_PKG_AUTHORS").unwrap(), "Literal $(location :absent)");
    assert!(std::path::Path::new(&std::env::var("CARGO_MANIFEST_PATH").expect("CARGO_MANIFEST_PATH")).is_file());
    let value = std::fs::read_to_string("value.txt").unwrap();
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    std::fs::write(out.join("value.rs"), format!("const VALUE:u32={value};")).unwrap();
    std::fs::write("src/generated.rs", format!("const GENERATED:u32={value};")).unwrap();
    std::fs::write(out.join("probe.rs"), "pub fn probe() -> String { String::new() }").unwrap();
    let probe = std::process::Command::new(std::env::var_os("RUSTC").unwrap())
        .arg("--crate-type=lib").arg("--emit=metadata").arg(out.join("probe.rs"))
        .arg("-o").arg(out.join("probe.rmeta")).output().unwrap();
    assert!(probe.status.success(), "{}", String::from_utf8_lossy(&probe.stderr));
    println!("cargo::rustc-check-cfg=cfg(generated)");
    println!("cargo::rustc-cfg=generated");
    println!("cargo::rustc-env=SCRIPT_VALUE={value}");
    println!("cargo::rerun-if-changed=value.txt");
}
"""

CONSUMER = """
include!(concat!(env!("OUT_DIR"), "/value.rs"));
include!("generated.rs");
const _:() = assert!(GENERATED == VALUE);
#[cfg(not(generated))] compile_error!("missing generated cfg");
fn main() {
    assert_eq!(VALUE.to_string(), env!("SCRIPT_VALUE"));
    println!("{}:{VALUE}", calc::value!());
}
"""


def qualify(project: Path, binary: str) -> None:
    """Require Cargo-compatible cfg, environment and generated-file behavior."""
    manifest = project / 'app/Cargo.toml'
    manifest.write_text(
        manifest.read_text().replace(
            '[dependencies]',
            'authors=["Literal $(location :absent)"]\n[features]\ndefault=["fast"]\nfast=[]\n[dependencies]',
        )
    )
    (project / '.cargo').mkdir()
    (project / '.cargo/config.toml').write_text(
        '[build]\nrustflags=["--cfg", "input_cfg"]\n'
    )
    (project / 'app/value.txt').write_text('11')
    (project / 'app/src/generated.rs').write_text('const GENERATED:u32=0;')
    (project / 'app/build.rs').write_text(SCRIPT)
    (project / 'app/src/main.rs').write_text(CONSUMER)
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
    assert reference.stdout.strip() == '7:11', reference.stdout
    (project / 'app/src/generated.rs').write_text('const GENERATED:u32=0;')
    assert build(project, binary, '7:11')
    assert build(project, binary, '7:11') == []
    assert (project / 'app/src/generated.rs').read_text() == 'const GENERATED:u32=0;'
    reuse(project, binary)
    (project / 'app/value.txt').write_text('13')
    assert build(project, binary, '7:13')
    profile(project, binary)
    refused = run(project, binary, 'build', 'app', '--console', 'simple')
    assert refused.returncode != 0, 'host execution must not reuse isolated scripts'
    assert 'verified declared-input executor' in refused.stderr, refused.stderr
    print(
        'ok  Cargo build scripts: reference parity, cfg, literal environment, generated files, edits, warm reuse, host refusal'
    )


def profile(project: Path, binary: str) -> None:
    """Preserve Cargo's inherited profile root and debugging settings."""
    manifest = project / 'Cargo.toml'
    manifest.write_text(
        manifest.read_text()
        + '\n[profile.probe]\ninherits="release"\nopt-level=0\nlto="off"\ndebug=1\ndebug-assertions=true\n'
    )
    script = project / 'app/build.rs'
    script.write_text(
        script.read_text().replace(
            'fn main() {',
            """fn main() {
        assert_eq!(std::env::var("PROFILE").unwrap(), "release");
        assert_eq!(std::env::var("DEBUG").unwrap(), "true");
        assert_eq!(std::env::var("OPT_LEVEL").unwrap(), "0");
        assert!(std::env::var("CARGO_CFG_DEBUG_ASSERTIONS").is_ok());
    """,
        )
    )
    reference = run(
        project,
        'rustup',
        'run',
        '1.97.1',
        'cargo',
        'run',
        '--locked',
        '--offline',
        '--profile',
        'probe',
        '-p',
        'app',
    )
    assert reference.returncode == 0, reference.stderr
    assert reference.stdout.strip() == '7:13', reference.stdout
    (project / 'app/src/generated.rs').write_text('const GENERATED:u32=0;')
    config = project / '.bsmr.local'
    config.write_text(config.read_text() + '\n[rust]\nprofile=probe\n')
    assert build(project, binary, '7:13')
    assert build(project, binary, '7:13') == []


def reuse(project: Path, binary: str) -> None:
    """Restore complete script outputs into a second checkout without compilation."""
    clone = project.with_name('clone')
    shutil.copytree(project, clone, ignore=shutil.ignore_patterns('bsmr-out', 'target'))
    try:
        actions = build(clone, binary, '7:11')
        assert not any('"executor":"Local"' in action for action in actions), actions
        assert any('"executor":"Cache"' in action for action in actions), actions
    finally:
        result = run(clone, binary, 'kill')
        assert result.returncode == 0, result.stderr


def main() -> None:
    """Use an explicit compiler binary and independently verified runtime."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('binary', type=Path)
    parser.add_argument('runtime', type=Path)
    args = parser.parse_args()
    binary = str(args.binary.resolve(strict=True))
    runtime = args.runtime.resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix='bsmr-scripts-') as temporary:
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
