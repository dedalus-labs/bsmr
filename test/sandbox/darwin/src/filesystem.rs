//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Check whether native executables can use a private Darwin root filesystem.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use anyhow::Context;
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
    shared_cache(&root)?;
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

/// Supply the OS libraries that modern macOS stores only in its shared cache.
fn shared_cache(root: &Path) -> Result<()> {
    let cache = crate::system::cache()?;
    let source = cache.parent().context("OS shared cache has no directory")?;
    let prefix = cache
        .file_name()
        .context("OS shared cache has no filename")?;
    println!(
        "{}",
        serde_json::json!({"case": "cache_source", "path": cache, "bytes": cache.metadata()?.len()})
    );
    let destination = root.join("System/Library/dyld");
    fs::create_dir_all(&destination)?;
    let mut bytes = 0;
    let mut files = 0;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        if entry
            .file_name()
            .as_encoded_bytes()
            .starts_with(prefix.as_encoded_bytes())
        {
            ensure!(
                entry.file_type()?.is_file(),
                "shared cache entry must be a file"
            );
            let size = entry.metadata()?.len();
            let copied = destination.join(entry.file_name());
            // APFS copies can report zero bytes. Validate the destination's logical size.
            fs::copy(entry.path(), &copied)?;
            ensure!(
                copied.metadata()?.len() == size,
                "shared cache copy is truncated"
            );
            bytes += size;
            files += 1;
        }
    }
    ensure!(
        bytes > 0,
        "the OS shared cache must not be empty: {files} files"
    );
    println!("{{\"case\":\"runtime\",\"shared_cache_bytes\":{bytes}}}");
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
