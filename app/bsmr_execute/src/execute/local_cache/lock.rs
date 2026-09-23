//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Serializes destructive collection against local-cache reads and publications.

use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::path::Path;

use super::io_error;

const LOCK_FILE: &str = "cache.lock";

/// Releases one process-safe cache lock when it leaves scope.
pub(super) struct CacheLock {
    file: File,
}

impl CacheLock {
    /// Acquires a shared lease for a lookup, publication, or materialization.
    pub(super) fn shared(root: &Path) -> bsmr_error::Result<Self> {
        let file = open(root)?;
        fs4::fs_std::FileExt::lock_shared(&file)
            .map_err(|error| io_error("lock cache for use", root, error))?;
        Ok(Self { file })
    }

    /// Tries to acquire collection ownership without waiting.
    #[cfg(test)]
    pub(super) fn try_exclusive(root: &Path) -> bsmr_error::Result<Option<Self>> {
        let file = open(root)?;
        match fs4::fs_std::FileExt::try_lock_exclusive(&file) {
            Ok(()) => Ok(Some(Self { file })),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
            Err(error) => Err(io_error("try lock cache for collection", root, error)),
        }
    }

    /// Acquires the exclusive lease required before deleting cache entries.
    pub(super) fn exclusive(root: &Path) -> bsmr_error::Result<Self> {
        let file = open(root)?;
        fs4::fs_std::FileExt::lock_exclusive(&file)
            .map_err(|error| io_error("lock cache for collection", root, error))?;
        Ok(Self { file })
    }
}

impl Drop for CacheLock {
    fn drop(&mut self) {
        if let Err(error) = fs4::fs_std::FileExt::unlock(&self.file) {
            tracing::warn!(%error, "local cache lock release failed");
        }
    }
}

/// Opens the one lock file shared by every process using this cache root.
fn open(root: &Path) -> bsmr_error::Result<File> {
    fs::create_dir_all(root).map_err(|error| io_error("create cache root", root, error))?;
    let path = root.join(LOCK_FILE);
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&path)
        .map_err(|error| io_error("open cache lock", path, error))
}
