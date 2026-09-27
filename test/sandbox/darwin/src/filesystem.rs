//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Check whether native executables can use a private Darwin root filesystem.

use std::fs;
use std::net::SocketAddr;
use std::net::TcpListener;
use std::net::TcpStream;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;

use crate::identity;

/// Execute the same binary below a new root with only its loader and one input.
pub(crate) fn check(parent: &Path, compiler: &Path) -> Result<()> {
    let root = parent.join("root");
    fs::create_dir(&root)?;
    fs::create_dir_all(root.join("usr/lib"))?;
    for directory in [&root, &root.join("usr"), &root.join("usr/lib")] {
        fs::set_permissions(directory, fs::Permissions::from_mode(0o755))?;
    }
    fs::copy("/usr/lib/dyld", root.join("usr/lib/dyld"))?;
    fs::create_dir(root.join("bin"))?;
    fs::copy("/usr/bin/sandbox-exec", root.join("bin/sandbox-exec"))?;
    shared_cache(&root)?;
    crate::compiler::stage(&root, compiler)?;
    fs::copy(std::env::current_exe()?, root.join("probe"))?;
    fs::write(root.join("input"), b"declared input")?;
    fs::set_permissions(root.join("input"), fs::Permissions::from_mode(0o644))?;
    fs::create_dir(root.join("output"))?;
    fs::set_permissions(root.join("output"), fs::Permissions::from_mode(0o1777))?;
    fs::write(root.join("output/input"), b"nested input")?;
    fs::set_permissions(root.join("output/input"), fs::Permissions::from_mode(0o444))?;
    let outside = parent.join("outside");
    fs::write(&outside, b"host input")?;
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let address = listener.local_addr()?;
    let connection = TcpStream::connect(address)?;
    drop(connection);
    let output = Command::new("/usr/sbin/chroot")
        .arg(&root)
        .arg("/bin/sandbox-exec")
        .args([
            "-p",
            "(version 1)(deny default)(allow file* process* sysctl-read system-socket)(allow signal (target same-sandbox))",
        ])
        .arg("/probe")
        .arg("view")
        .arg(&outside)
        .arg(address.to_string())
        .env_clear()
        .output()?;
    ensure!(
        output.status.success(),
        "native root execution failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    ensure!(output.stdout == b"private root passed\n");
    println!(
        "{{\"case\":\"compiler\",\"metadata_bytes\":{}}}",
        fs::metadata(root.join("output/libsource.rmeta"))?.len()
    );
    println!(
        "{{\"case\":\"filesystem\",\"host_read_denied\":true,\"network_denied\":true,\"inputs_readonly\":true}}"
    );
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
pub(crate) fn view(outside: &Path, address: SocketAddr) -> Result<()> {
    identity::enter()?;
    crate::compiler::run()?;
    ensure!(fs::read("/input")? == b"declared input");
    ensure!(fs::read("/output/input")? == b"nested input");
    let result = fs::read(outside);
    ensure!(
        matches!(result, Err(error) if error.kind() == std::io::ErrorKind::NotFound),
        "host path must be absent from the private root"
    );
    for result in [
        fs::write("/input", b"changed"),
        fs::write("/output/input", b"changed"),
        fs::remove_file("/output/input"),
    ] {
        ensure!(
            matches!(result, Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied),
            "declared inputs must remain read-only under a writable parent"
        );
    }
    fs::write("/output/result", b"produced output")?;
    let network = TcpStream::connect_timeout(&address, Duration::from_secs(1));
    ensure!(
        matches!(network, Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied),
        "sandbox must deny a reachable network peer"
    );
    println!("private root passed");
    Ok(())
}
