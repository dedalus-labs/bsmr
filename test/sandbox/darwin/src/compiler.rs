//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Qualify the real compiler in the private root before adding native build orchestration.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use anyhow::Result;
use anyhow::ensure;

/// Resolve the pinned compiler as the ordinary runner user, before sudo changes HOME.
pub(crate) fn record(destination: &Path) -> Result<()> {
    let output = Command::new("rustup")
        .args(["run", "1.98.0", "rustc", "--print", "sysroot"])
        .output()?;
    ensure!(output.status.success(), "rustc sysroot lookup failed");
    fs::write(destination, output.stdout)?;
    Ok(())
}

/// Copy the trusted qualification toolchain into the disposable filesystem.
pub(crate) fn stage(root: &Path, record: &Path) -> Result<()> {
    let source = fs::read_to_string(record)?;
    let source = Path::new(source.trim());
    ensure!(source.is_absolute(), "compiler sysroot must be absolute");
    let destination = root.join("toolchain");
    fs::create_dir_all(destination.join("bin"))?;
    copy(&source.join("bin/rustc"), &destination.join("bin/rustc"))?;
    copy(&source.join("lib"), &destination.join("lib"))?;
    fs::write(root.join("source.rs"), "pub fn answer() -> u64 { 42 }\n")?;
    fs::set_permissions(root.join("source.rs"), fs::Permissions::from_mode(0o444))?;
    Ok(())
}

/// Copy trusted runtime bytes as root-owned files without write or privilege bits.
pub(crate) fn copy(source: &Path, destination: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(source)?;
    if metadata.is_dir() {
        fs::create_dir(destination)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            copy(&entry.path(), &destination.join(entry.file_name()))?;
        }
        fs::set_permissions(destination, fs::Permissions::from_mode(0o555))?;
    } else if metadata.is_file() {
        fs::copy(source, destination)?;
        // Privileged APFS clones otherwise retain the original user's ownership.
        std::os::unix::fs::chown(destination, Some(0), Some(0))?;
        ensure!(
            destination.metadata()?.len() == metadata.len(),
            "truncated toolchain file"
        );
        let mode = if metadata.permissions().mode() & 0o111 == 0 {
            0o444
        } else {
            0o555
        };
        fs::set_permissions(destination, fs::Permissions::from_mode(mode))?;
    } else {
        ensure!(
            metadata.is_symlink(),
            "unsupported toolchain entry: {source:?}"
        );
        std::os::unix::fs::symlink(fs::read_link(source)?, destination)?;
    }
    Ok(())
}

/// Compile real Rust metadata after the probe has dropped its identity and entered Seatbelt.
pub(crate) fn run() -> Result<()> {
    let output = Command::new("/toolchain/bin/rustc")
        .args([
            "--crate-type=lib",
            "--emit=metadata",
            "/source.rs",
            "--out-dir",
            "/output",
        ])
        .current_dir("/")
        .env_clear()
        .env("TMPDIR", "/output")
        .output()?;
    ensure!(
        output.status.success(),
        "isolated rustc failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    ensure!(fs::metadata("/output/libsource.rmeta")?.len() > 0);
    Ok(())
}
