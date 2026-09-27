//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Require detached writers to stop before their output can be accepted.

use std::fs::File;
use std::fs::{self};
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::Child;
use std::process::Command;
use std::process::Stdio;
use std::thread;
use std::time::Duration;
use std::time::Instant;

use anyhow::Result;
use anyhow::ensure;
use bsmr_native::identity::Identity;
use bsmr_native::identity::occupied;
use nix::unistd::Gid;
use nix::unistd::Uid;
use nix::unistd::chown;
use nix::unistd::setsid;

use crate::identity::ID;
use crate::identity::acquire;
use crate::identity::enter;

/// Every writer exits even if the supervising test fails.
const WRITER_LIFETIME: Duration = Duration::from_secs(12);

/// Write after the launching process exits, with an independent finite lifetime.
pub(crate) fn writer(path: &Path) -> Result<()> {
    ensure!(
        Uid::current().as_raw() == ID && Uid::effective().as_raw() == ID,
        "writer has wrong UID"
    );
    setsid()?;
    // SAFETY: SIGTERM is intentionally ignored only in this bounded, disposable child.
    unsafe { libc::signal(libc::SIGTERM, libc::SIG_IGN) };
    let mut heartbeat = File::create(path.join("heartbeat"))?;
    let deadline = Instant::now() + WRITER_LIFETIME;
    while Instant::now() < deadline {
        heartbeat.write_all(b".")?;
        heartbeat.flush()?;
        thread::sleep(Duration::from_millis(20));
    }
    Ok(())
}

/// Launch a detached writer after dropping credentials, then exit or await cancellation.
pub(crate) fn workload(path: &Path, cancel: bool) -> Result<()> {
    enter()?;
    Command::new(std::env::current_exe()?)
        .arg("writer")
        .arg(path)
        .env_clear()
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    ready(path)?;
    if cancel {
        thread::sleep(WRITER_LIFETIME);
    }
    Ok(())
}

/// Wait for the finite writer's first observable effect.
fn ready(path: &Path) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(3);
    while !path.join("heartbeat").exists() {
        ensure!(Instant::now() < deadline, "writer did not become ready");
        thread::sleep(Duration::from_millis(5));
    }
    Ok(())
}

/// Observe a writer after its launcher exits, then require UID cleanup to stop it.
fn case(lease: &Identity, root: &Path, name: &str) -> Result<()> {
    let path = root.join(name);
    fs::create_dir(&path)?;
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?;
    chown(&path, Some(Uid::from_raw(ID)), Some(Gid::from_raw(ID)))?;
    let mut child: Child = Command::new(root.join("probe"))
        .arg(name)
        .arg(&path)
        .process_group(0)
        .env_clear()
        .stdin(Stdio::null())
        .spawn()?;
    ready(&path)?;
    if name == "cancel" {
        child.kill()?;
    }
    child.wait()?;
    let before = fs::metadata(path.join("heartbeat"))?.len();
    thread::sleep(Duration::from_millis(100));
    ensure!(
        fs::metadata(path.join("heartbeat"))?.len() > before,
        "detached-writer control did not reproduce"
    );
    let started = Instant::now();
    lease.drain()?;
    let elapsed = started.elapsed();
    let final_size = fs::metadata(path.join("heartbeat"))?.len();
    thread::sleep(Duration::from_millis(100));
    ensure!(
        fs::metadata(path.join("heartbeat"))?.len() == final_size,
        "output changed after UID drained"
    );
    ensure!(!occupied(ID)?, "UID membership returned after drain");
    println!(
        "{}",
        serde_json::json!({"case": name, "uid": ID, "cleanup_ms": elapsed.as_secs_f64()*1000.0, "empty": true, "stable_output": true})
    );
    Ok(())
}

/// Keep the root operation bounded to an unused UID and a newly created evidence directory.
pub(crate) fn run(root: &Path) -> Result<()> {
    ensure!(
        root.parent() == Some(Path::new("/private/var/tmp")),
        "evidence must be directly under /private/var/tmp"
    );
    let lease = acquire()?;
    fs::create_dir(root)?;
    fs::set_permissions(root, fs::Permissions::from_mode(0o711))?;
    fs::copy(std::env::current_exe()?, root.join("probe"))?;
    fs::set_permissions(root.join("probe"), fs::Permissions::from_mode(0o555))?;
    for name in ["normal", "cancel"] {
        case(&lease, root, name)?;
    }
    lease.drain()?;
    crate::filesystem::check(root)?;
    Ok(())
}
