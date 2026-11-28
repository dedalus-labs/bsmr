//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Persists verified official Go SDK identities in the user root.

//! Reads and writes the per-version records that package evaluation lowers into Go toolchain
//! targets.
//!
//! Records live outside every checkout, and a valid record is never replaced, so later official
//! metadata for the same version cannot silently change the SDK identity.

use std::fs;
use std::io::Write as _;
use std::path::Path;
use std::path::PathBuf;

use super::GoToolchainError;
use super::GoToolchainLock;
use super::validate_lock;

/// Returns the valid record for `version`, or `None` when it is absent or invalid.
pub(crate) fn read_metadata(
    directory: &Path,
    version: &str,
) -> Result<Option<GoToolchainLock>, GoToolchainError> {
    let path = metadata_path(directory, version);
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(GoToolchainError::Read {
                path,
                message: error.to_string(),
            });
        }
    };
    Ok(serde_json::from_slice::<GoToolchainLock>(&bytes)
        .ok()
        .filter(|lock| lock.version == version && validate_lock(lock).is_ok()))
}

/// Returns the valid record for `version` and fails closed, naming the file, without one.
pub(crate) fn require_metadata(
    directory: &Path,
    version: &str,
) -> Result<GoToolchainLock, GoToolchainError> {
    read_metadata(directory, version)?
        .ok_or_else(|| GoToolchainError::Metadata(metadata_path(directory, version)))
}

/// Records verified official metadata for one version in the user root.
pub(crate) fn write_metadata(
    directory: &Path,
    lock: &GoToolchainLock,
) -> Result<(), GoToolchainError> {
    validate_lock(lock)?;
    let mut content = serde_json::to_string_pretty(lock)
        .map_err(|error| GoToolchainError::Lock(error.to_string()))?;
    content.push('\n');
    fs::create_dir_all(directory).map_err(|error| GoToolchainError::Write {
        path: directory.to_owned(),
        message: error.to_string(),
    })?;
    atomic_write(&metadata_path(directory, &lock.version), content.as_bytes())
}

/// Names the record that package evaluation selects for the `go.mod` pin `version`.
fn metadata_path(directory: &Path, version: &str) -> PathBuf {
    directory.join(format!("go{version}.json"))
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
