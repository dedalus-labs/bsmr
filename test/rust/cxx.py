# ===----------------------------------------------------------------------===
# Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
# SPDX-License-Identifier: Apache-2.0
# ===----------------------------------------------------------------------===

# Verifies that Cargo build scripts can use the selected native compiler family.

import argparse
from pathlib import Path
import tempfile

from macros import build, initialize, run


SCRIPT = r"""
fn main() {
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap());
    for (compiler, source) in [("CC", "value.c"), ("CXX", "value.cpp")] {
        let result = std::process::Command::new(std::env::var_os(compiler).unwrap())
            .arg("-c").arg(source).arg("-o").arg(out.join(format!("{source}.o")))
            .output().unwrap();
        assert!(result.status.success(), "{compiler}: {}", String::from_utf8_lossy(&result.stderr));
    }
    let result = std::process::Command::new(std::env::var_os("AR").unwrap())
        .arg("crs").arg(out.join("libvalue.a"))
        .arg(out.join("value.c.o")).arg(out.join("value.cpp.o"))
        .output().unwrap();
    assert!(result.status.success(), "AR: {}", String::from_utf8_lossy(&result.stderr));
    println!("cargo::rustc-link-search=native={}", out.display());
    println!("cargo::rustc-link-lib=static=value");
    println!("cargo::rerun-if-changed=value.c");
    println!("cargo::rerun-if-changed=value.cpp");
}
"""


def qualify(project: Path, binary: str) -> None:
    """Compile C and C++ through the rule's actual wrappers, then link and run Rust."""
    (project / 'app/Cargo.toml').write_text(
        '[package]\nname="app"\nversion="0.1.0"\nedition="2024"\n'
    )
    (project / 'app/build.rs').write_text(SCRIPT)
    (project / 'app/value.c').write_text('int c_value(void) { return 7; }\n')
    (project / 'app/value.cpp').write_text(
        'extern "C" int cpp_value() { return 11; }\n'
    )
    (project / 'app/src/main.rs').write_text(
        'unsafe extern "C" { fn c_value() -> i32; fn cpp_value() -> i32; }\n'
        'fn main() { println!("{}", unsafe { c_value() + cpp_value() }); }\n'
    )
    locked = run(
        project, 'rustup', 'run', '1.97.1', 'cargo', 'generate-lockfile', '--offline'
    )
    assert locked.returncode == 0, locked.stderr
    assert build(project, binary, '18')
    assert build(project, binary, '18') == []
    (project / 'app/value.c').write_text('int c_value(void) { return 13; }\n')
    assert build(project, binary, '24')
    print(
        'ok  native compilers: C, C++, archive, Rust linking, warm reuse, source edit'
    )


def main() -> None:
    """Require an explicit binary and isolated runtime containing the native compilers."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('binary', type=Path)
    parser.add_argument('runtime', type=Path)
    args = parser.parse_args()
    binary = str(args.binary.resolve(strict=True))
    runtime = args.runtime.resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix='bsmr-cxx-') as temporary:
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
