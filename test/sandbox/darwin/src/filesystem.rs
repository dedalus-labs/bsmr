//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Check whether native executables can use a private Darwin root filesystem.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use anyhow::Result;
use anyhow::ensure;

use crate::identity;

/// Execute the same binary below a new root with only its loader and one input.
pub(crate) fn check(parent: &Path) -> Result<()> {
    let root = parent.join("root");
    fs::create_dir(&root)?;
    fs::create_dir_all(root.join("usr/lib"))?;
    for directory in [&root, &root.join("usr"), &root.join("usr/lib")] {
        fs::set_permissions(directory, fs::Permissions::from_mode(0o755))?;
    }
    fs::copy("/usr/lib/dyld", root.join("usr/lib/dyld"))?;
    fs::copy(std::env::current_exe()?, root.join("probe"))?;
    fs::write(root.join("input"), b"declared input")?;
    fs::set_permissions(root.join("input"), fs::Permissions::from_mode(0o644))?;
    let outside = parent.join("outside");
    fs::write(&outside, b"host input")?;
    let output = Command::new("/usr/sbin/chroot")
        .arg(&root)
        .arg("/probe")
        .arg("view")
        .arg(&outside)
        .env_clear()
        .output()?;
    ensure!(
        output.status.success(),
        "native root execution failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    ensure!(output.stdout == b"private root passed\n");
    println!("{{\"case\":\"filesystem\",\"host_read_denied\":true}}");
    Ok(())
}

/// Require both the declared input and denial of a known host file after credential drop.
pub(crate) fn view(outside: &Path) -> Result<()> {
    identity::enter()?;
    ensure!(fs::read("/input")? == b"declared input");
    let result = fs::read(outside);
    ensure!(
        matches!(result, Err(error) if error.kind() == std::io::ErrorKind::NotFound),
        "host path must be absent from the private root"
    );
    println!("private root passed");
    Ok(())
}
