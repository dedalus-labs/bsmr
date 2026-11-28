//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Persists the committed lock for one official Go SDK.

//! Writes or verifies the lock that package evaluation lowers into Go toolchain targets.
//!
//! The lock is the only committed toolchain artifact: `toolchains//` reads it when it is
//! evaluated, so no generated Starlark can drift from it.

use std::fs;
use std::io::Write as _;
use std::path::Path;

use super::GoToolchainError;
use super::GoToolchainLock;
use super::LOCK_FILE;
use super::validate_lock;

/// Writes or verifies the lock at the project root without touching a user-owned file.
pub(crate) fn write_lock(
    root: &Path,
    lock: &GoToolchainLock,
    check: bool,
) -> Result<(), GoToolchainError> {
    let path = root.join(LOCK_FILE);
    let content = render(lock)?;
    validate_owned_lock(&path)?;
    let current = fs::read(&path).ok();
    if current.as_deref() == Some(content.as_bytes()) {
        return Ok(());
    }
    if check {
        return Err(GoToolchainError::Stale(path));
    }
    atomic_write(&path, content.as_bytes())
}

/// Writes or verifies the user-root metadata for the locked version.
///
/// Metadata outlives every checkout, so a valid file recording other archives for the same
/// version is refused rather than replaced.
pub(crate) fn write_metadata(
    directory: &Path,
    lock: &GoToolchainLock,
    check: bool,
) -> Result<(), GoToolchainError> {
    let path = directory.join(format!("go{}.json", lock.version));
    let content = render(lock)?;
    match fs::read(&path) {
        Ok(current) if current == content.as_bytes() => return Ok(()),
        Ok(current) => {
            if serde_json::from_slice::<GoToolchainLock>(&current).is_ok_and(|current| {
                current.version == lock.version && validate_lock(&current).is_ok()
            }) {
                return Err(GoToolchainError::Conflict(path));
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(GoToolchainError::Read {
                path,
                message: error.to_string(),
            });
        }
    }
    if check {
        return Err(GoToolchainError::Stale(path));
    }
    fs::create_dir_all(directory).map_err(|error| GoToolchainError::Write {
        path: directory.to_owned(),
        message: error.to_string(),
    })?;
    atomic_write(&path, content.as_bytes())
}

/// Renders the canonical bytes shared by the lock and the user-root metadata.
fn render(lock: &GoToolchainLock) -> Result<String, GoToolchainError> {
    validate_lock(lock)?;
    let mut content = serde_json::to_string_pretty(lock)
        .map_err(|error| GoToolchainError::Lock(error.to_string()))?;
    content.push('\n');
    Ok(content)
}

/// Allows only a missing or parseable Bessemer-owned lock.
fn validate_owned_lock(path: &Path) -> Result<(), GoToolchainError> {
    match fs::read(path) {
        Ok(bytes) => {
            let lock = serde_json::from_slice::<serde_json::Value>(&bytes)
                .map_err(|_| GoToolchainError::UserOwned(path.to_owned()))?;
            if lock.get("generated_by").and_then(serde_json::Value::as_str)
                == Some(super::GENERATED_BY)
            {
                Ok(())
            } else {
                Err(GoToolchainError::UserOwned(path.to_owned()))
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(GoToolchainError::Read {
            path: path.to_owned(),
            message: error.to_string(),
        }),
    }
}

/// Atomically persists one generated file beside its final destination.
fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), GoToolchainError> {
    let parent = path.parent().ok_or_else(|| GoToolchainError::Write {
        path: path.to_owned(),
        message: "path has no parent".to_owned(),
    })?;
    let mut temporary =
        tempfile::NamedTempFile::new_in(parent).map_err(|error| GoToolchainError::Write {
            path: path.to_owned(),
            message: error.to_string(),
        })?;
    temporary
        .write_all(bytes)
        .map_err(|error| GoToolchainError::Write {
            path: path.to_owned(),
            message: error.to_string(),
        })?;
    temporary
        .persist(path)
        .map_err(|error| GoToolchainError::Write {
            path: path.to_owned(),
            message: error.error.to_string(),
        })?;
    Ok(())
}
