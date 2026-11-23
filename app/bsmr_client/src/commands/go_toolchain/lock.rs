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
    validate_lock(lock)?;
    let path = root.join(LOCK_FILE);
    let mut content = serde_json::to_string_pretty(lock)
        .map_err(|error| GoToolchainError::Lock(error.to_string()))?;
    content.push('\n');
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

/// Publishes `lock` while `install` acquires it, restoring the previous lock if that fails.
///
/// The `toolchains//` package reads the lock to materialize the SDK archive, so the lock must be
/// on disk during installation. A failed installation must not leave a new lock advertising tools
/// that were never installed; consumers then still see the previous, consistent state.
pub(crate) fn publish_lock<E: From<GoToolchainError>>(
    root: &Path,
    lock: &GoToolchainLock,
    install: impl FnOnce() -> Result<(), E>,
) -> Result<(), E> {
    let path = root.join(LOCK_FILE);
    let previous = match fs::read(&path) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            return Err(GoToolchainError::Read {
                path,
                message: error.to_string(),
            }
            .into());
        }
    };
    write_lock(root, lock, false)?;
    let installed = install();
    if installed.is_err() {
        match previous {
            Some(bytes) => atomic_write(&path, &bytes)?,
            None => fs::remove_file(&path).map_err(|error| GoToolchainError::Write {
                path: path.clone(),
                message: error.to_string(),
            })?,
        }
    }
    installed
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
