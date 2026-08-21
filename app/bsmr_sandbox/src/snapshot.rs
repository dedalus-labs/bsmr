//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Creates the single pristine VM state resumed by untrusted-v1 actions.

use std::fs;
use std::fs::File;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::path::PathBuf;
use std::process::Child;
use std::process::Command;
use std::process::Stdio;
use std::time::Duration;
use std::time::Instant;

use thiserror::Error;

use crate::MAX_ACTION_BYTES;
use crate::MAX_INPUT_BYTES;
use crate::MAX_OUTPUT_BYTES;
use crate::MEMORY_MIB;
use crate::VCPU_COUNT;
use crate::firecracker::ApiClient;

pub const READY_MARKER: &str = "bsmr-sandbox-guest: pristine-ready";
pub const GUEST_READY_BYTE: u8 = 0x52;
pub const WAKE_PORT: u32 = 18_345;
pub const WAKE_BYTE: u8 = 0x42;

const BUILD_TIMEOUT: Duration = Duration::from_secs(30);
const POLL_INTERVAL: Duration = Duration::from_millis(5);
const MAX_LOG_BYTES: u64 = 64 * 1024;
const TRANSIENTS: [&str; 6] = [
    "action",
    "input",
    "output",
    "api.socket",
    "vsock.socket",
    "snapshot.log",
];

/// A fail-closed pristine-snapshot build failure.
#[derive(Debug, Error)]
pub enum SnapshotError {
    #[error("snapshot output already exists: {0:?}")]
    Exists(PathBuf),
    #[error("snapshot I/O failure at {path:?}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to start Firecracker: {0}")]
    Spawn(#[source] std::io::Error),
    #[error("Firecracker API failure: {0}")]
    Api(#[from] crate::firecracker::ApiError),
    #[error("Firecracker exited before the pristine guest barrier: {0}")]
    EarlyExit(std::process::ExitStatus),
    #[error("Firecracker did not reach the pristine guest barrier")]
    Timeout,
    #[error("snapshot-source process is missing after successful startup")]
    MissingProcess,
    #[error("Firecracker snapshot output is empty: {0:?}")]
    Empty(PathBuf),
    #[error("snapshot cleanup failed: {0}")]
    Cleanup(String),
}

/// Boots trusted guest code once and captures it before any action executes.
pub fn create(directory: &Path) -> Result<(), SnapshotError> {
    for name in TRANSIENTS.into_iter().chain(["snapshot", "memory"]) {
        let path = directory.join(name);
        match fs::symlink_metadata(&path) {
            Ok(_) => return Err(SnapshotError::Exists(path)),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => return Err(io_error(path, source)),
        }
    }
    let mut build = SnapshotBuild::new(directory);
    build.start()?;
    let api_path = directory.join("api.socket");
    wait_for_path(build.child()?, &api_path, BUILD_TIMEOUT)?;
    configure(&ApiClient::new(&api_path, BUILD_TIMEOUT))?;
    wait_for_marker(
        build.child()?,
        &directory.join("snapshot.log"),
        BUILD_TIMEOUT,
    )?;
    let api = ApiClient::new(&api_path, BUILD_TIMEOUT);
    api.patch("/vm", &serde_json::json!({"state": "Paused"}))?;
    api.put(
        "/snapshot/create",
        &serde_json::json!({
            "snapshot_type": "Full",
            "snapshot_path": "snapshot",
            "mem_file_path": "memory"
        }),
    )?;
    build.finish()
}

/// Configures the exact device graph serialized into the pristine snapshot.
fn configure(api: &ApiClient<'_>) -> Result<(), SnapshotError> {
    api.put(
        "/machine-config",
        &serde_json::json!({
            "vcpu_count": VCPU_COUNT,
            "mem_size_mib": MEMORY_MIB,
            "smt": false,
            "track_dirty_pages": false
        }),
    )?;
    api.put(
        "/boot-source",
        &serde_json::json!({
            "kernel_image_path": "kernel",
            "boot_args": "root=/dev/vda ro console=ttyS0 reboot=k panic=1 pci=off init=/sbin/bsmr-sandbox-guest"
        }),
    )?;
    for (id, path, root, read_only) in [
        ("rootfs", "rootfs", true, true),
        ("input", "input", false, true),
        ("output", "output", false, false),
        ("action", "action", false, true),
    ] {
        api.put(
            &format!("/drives/{id}"),
            &serde_json::json!({
                "drive_id": id,
                "path_on_host": path,
                "is_root_device": root,
                "is_read_only": read_only
            }),
        )?;
    }
    api.put(
        "/vsock",
        &serde_json::json!({
            "vsock_id": "bsmr-control",
            "guest_cid": 3,
            "uds_path": "vsock.socket"
        }),
    )?;
    api.put("/entropy", &serde_json::json!({}))?;
    api.put(
        "/actions",
        &serde_json::json!({"action_type": "InstanceStart"}),
    )?;
    Ok(())
}

/// Creates one sparse, fixed-capacity snapshot transport device.
fn create_transport(directory: &Path, name: &str, bytes: u64) -> Result<(), SnapshotError> {
    let path = directory.join(name);
    File::options()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .and_then(|file| file.set_len(bytes))
        .map_err(|source| io_error(path, source))
}

/// Waits for the Firecracker API socket while proving the VMM remains alive.
fn wait_for_path(child: &mut Child, path: &Path, timeout: Duration) -> Result<(), SnapshotError> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if path.exists() {
            return Ok(());
        }
        if let Some(status) = child.try_wait().map_err(SnapshotError::Spawn)? {
            return Err(SnapshotError::EarlyExit(status));
        }
        std::thread::sleep(POLL_INTERVAL);
    }
    Err(SnapshotError::Timeout)
}

/// Waits until PID 1 has bound vsock and has not inspected any action state.
fn wait_for_marker(child: &mut Child, log: &Path, timeout: Duration) -> Result<(), SnapshotError> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        match fs::metadata(log) {
            Ok(metadata) if metadata.len() > MAX_LOG_BYTES => return Err(SnapshotError::Timeout),
            Ok(_)
                if fs::read_to_string(log)
                    .map_err(|source| io_error(log, source))?
                    .contains(READY_MARKER) =>
            {
                return Ok(());
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(source) => return Err(io_error(log, source)),
        }
        if let Some(status) = child.try_wait().map_err(SnapshotError::Spawn)? {
            return Err(SnapshotError::EarlyExit(status));
        }
        std::thread::sleep(POLL_INTERVAL);
    }
    Err(SnapshotError::Timeout)
}

/// Owns the temporary VMM and removes incomplete outputs on every failure path.
struct SnapshotBuild {
    directory: PathBuf,
    child: Option<Child>,
    complete: bool,
}

impl SnapshotBuild {
    /// Owns all create-only artifacts before the first one is written.
    fn new(directory: &Path) -> Self {
        Self {
            directory: directory.to_owned(),
            child: None,
            complete: false,
        }
    }

    /// Creates fixed transports and starts the trusted snapshot-source VMM.
    fn start(&mut self) -> Result<(), SnapshotError> {
        create_transport(&self.directory, "action", MAX_ACTION_BYTES)?;
        create_transport(&self.directory, "input", MAX_INPUT_BYTES)?;
        create_transport(&self.directory, "output", MAX_OUTPUT_BYTES)?;
        let log_path = self.directory.join("snapshot.log");
        let log = File::options()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&log_path)
            .map_err(|source| io_error(&log_path, source))?;
        let child = Command::new(self.directory.join("firecracker"))
            .arg("--api-sock")
            .arg("api.socket")
            .current_dir(&self.directory)
            .env_clear()
            .stdin(Stdio::null())
            .stdout(Stdio::from(
                log.try_clone()
                    .map_err(|source| io_error(&log_path, source))?,
            ))
            .stderr(Stdio::from(log))
            .spawn()
            .map_err(SnapshotError::Spawn)?;
        self.child = Some(child);
        Ok(())
    }

    /// Returns the live source VMM after successful startup.
    fn child(&mut self) -> Result<&mut Child, SnapshotError> {
        self.child.as_mut().ok_or(SnapshotError::MissingProcess)
    }

    /// Terminates the source VM and commits only non-empty snapshot outputs.
    fn finish(mut self) -> Result<(), SnapshotError> {
        terminate(self.child()?)?;
        for output in ["snapshot", "memory"] {
            let path = self.directory.join(output);
            let metadata = fs::metadata(&path).map_err(|source| io_error(&path, source))?;
            if metadata.len() == 0 {
                return Err(SnapshotError::Empty(path));
            }
        }
        cleanup_transients(&self.directory)?;
        self.complete = true;
        Ok(())
    }
}

impl Drop for SnapshotBuild {
    /// Kills the source VM and removes every partial or transient artifact.
    fn drop(&mut self) {
        let mut failures = Vec::new();
        if let Some(child) = &mut self.child
            && let Err(error) = terminate(child)
        {
            failures.push(error.to_string());
        }
        if let Err(error) = cleanup_transients(&self.directory) {
            failures.push(error.to_string());
        }
        if !self.complete {
            for output in ["snapshot", "memory"] {
                remove_if_present(&self.directory.join(output), &mut failures);
            }
        }
        if !failures.is_empty() && !std::thread::panicking() {
            eprintln!(
                "bsmr-sandbox-bundle: cleanup failed: {}",
                failures.join("; ")
            );
        }
    }
}

/// Kills and reaps the trusted snapshot-source process exactly once.
fn terminate(child: &mut Child) -> Result<(), SnapshotError> {
    if child.try_wait().map_err(SnapshotError::Spawn)?.is_some() {
        return Ok(());
    }
    child.kill().map_err(SnapshotError::Spawn)?;
    child.wait().map_err(SnapshotError::Spawn)?;
    Ok(())
}

/// Removes fixed build-only files without touching snapshot outputs.
fn cleanup_transients(directory: &Path) -> Result<(), SnapshotError> {
    let mut failures = Vec::new();
    for name in TRANSIENTS {
        remove_if_present(&directory.join(name), &mut failures);
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(SnapshotError::Cleanup(failures.join("; ")))
    }
}

/// Removes one known file while preserving all cleanup failures.
fn remove_if_present(path: &Path, failures: &mut Vec<String>) {
    match fs::remove_file(path) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => failures.push(io_error(path, error).to_string()),
    }
}

/// Attaches one snapshot path to an I/O failure.
fn io_error(path: impl Into<PathBuf>, source: std::io::Error) -> SnapshotError {
    SnapshotError::Io {
        path: path.into(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use super::MAX_ACTION_BYTES;
    use super::MAX_INPUT_BYTES;
    use super::MAX_OUTPUT_BYTES;
    use super::create_transport;

    /// Snapshot transports have one fixed device shape and are create-only.
    #[test]
    fn transport_shape_is_fixed() {
        let directory = tempfile::tempdir().unwrap();
        for (name, bytes) in [
            ("action", MAX_ACTION_BYTES),
            ("input", MAX_INPUT_BYTES),
            ("output", MAX_OUTPUT_BYTES),
        ] {
            create_transport(directory.path(), name, bytes).unwrap();
            assert_eq!(
                std::fs::metadata(directory.path().join(name))
                    .unwrap()
                    .len(),
                bytes
            );
            assert!(create_transport(directory.path(), name, bytes).is_err());
        }
    }
}
