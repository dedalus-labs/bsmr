//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

use std::process::Command;

#[test]
fn cache_transport_requires_neither_project_nor_daemon() {
    let temporary = tempfile::tempdir().expect("temporary directory");
    let source = temporary.path().join("source");
    let package = temporary.path().join("package");
    let destination = temporary.path().join("destination");
    let binary = env!("CARGO_BIN_EXE_bsmr");

    let export = Command::new(binary)
        .current_dir(temporary.path())
        .env("BSMR_NO_BSMRD", "true")
        .env("BSMR_LOCAL_CACHE_DIR", &source)
        .args(["cache", "export", "--output"])
        .arg(&package)
        .output()
        .expect("run cache export");
    assert!(
        export.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&export.stdout),
        String::from_utf8_lossy(&export.stderr)
    );

    let import = Command::new(binary)
        .current_dir(temporary.path())
        .env("BSMR_NO_BSMRD", "true")
        .env("BSMR_LOCAL_CACHE_DIR", &destination)
        .args(["cache", "import", "--input"])
        .arg(&package)
        .output()
        .expect("run cache import");
    assert!(
        import.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&import.stdout),
        String::from_utf8_lossy(&import.stderr)
    );

    assert!(package.join("manifest.json").is_file());
    assert!(package.join("payload").is_dir());
    assert!(destination.is_dir());
    assert!(!temporary.path().join("bsmr-out").exists());
}
