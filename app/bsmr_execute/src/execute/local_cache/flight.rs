//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Coordinates cache misses without holding an I/O permit while another process runs.

use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::fs::TryLockError;
use std::path::Path;
use std::sync::Arc;

use super::LocalActionCache;
use super::LocalActionResult;
use super::LocalCacheError;
use super::io_error;
use crate::execute::action_digest::ActionDigest;

/// Returns a published result or ownership of the missing action.
pub enum LocalActionReservation {
    Hit {
        result: LocalActionResult,
        pin: Arc<LocalActionPin>,
    },
    Lease(LocalActionLease),
}

/// Closing this file releases the process lock, including on execution failure.
#[derive(Debug)]
pub struct LocalActionLease {
    _file: File,
}

/// A lock on one exact action root retained by materialization or collection.
#[derive(Debug)]
pub struct LocalActionPin {
    _file: File,
}

impl LocalActionCache {
    /// Returns `None` immediately when another process owns the action's lock.
    pub fn try_reserve_action(
        &self,
        action: &ActionDigest,
    ) -> bsmr_error::Result<Option<LocalActionReservation>> {
        let Some(lease) = self.try_reserve_flight(action)? else {
            return Ok(None);
        };
        let pin = self.pin_action(action)?;
        let reservation = match (self.action_result(action)?, pin) {
            (Some(result), Some(pin)) => LocalActionReservation::Hit { result, pin },
            (None, _) => LocalActionReservation::Lease(lease),
            (Some(_), None) => {
                return Err(LocalCacheError::MissingActionPin(self.action_path(action)).into());
            }
        };
        Ok(Some(reservation))
    }

    /// Pins one existing action root against collection.
    pub fn pin_action(
        &self,
        action: &ActionDigest,
    ) -> bsmr_error::Result<Option<Arc<LocalActionPin>>> {
        let path = self.action_path(action);
        let file = match OpenOptions::new().read(true).open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(io_error("open action root for pin", path, error)),
        };
        fs4::fs_std::FileExt::lock_shared(&file)
            .map_err(|error| io_error("pin action root", path, error))?;
        Ok(Some(Arc::new(LocalActionPin { _file: file })))
    }

    /// Tries to lock one exact action root for collection.
    pub(super) fn try_collect_action_path(
        &self,
        action_path: &Path,
    ) -> bsmr_error::Result<Option<Arc<LocalActionPin>>> {
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .open(action_path)
            .map_err(|error| io_error("open action root for collection", action_path, error))?;
        match fs4::fs_std::FileExt::try_lock_exclusive(&file) {
            Ok(()) => Ok(Some(Arc::new(LocalActionPin { _file: file }))),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
            Err(error) => Err(io_error(
                "lock action root for collection",
                action_path,
                error,
            )),
        }
    }

    /// Acquires the bounded flight shard for one action miss or hit lookup.
    fn try_reserve_flight(
        &self,
        action: &ActionDigest,
    ) -> bsmr_error::Result<Option<LocalActionLease>> {
        let path = action_lock_path(&self.root, &self.action_path(action));
        let directory = path.parent().expect("action lock paths have a parent");
        fs::create_dir_all(directory)
            .map_err(|error| io_error("create action lock directory", directory, error))?;
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .map_err(|error| io_error("open action lock", &path, error))?;
        match file.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => return Ok(None),
            Err(TryLockError::Error(error)) => {
                return Err(io_error("acquire action lock", &path, error));
            }
        }
        Ok(Some(LocalActionLease { _file: file }))
    }
}

/// Maps every action root to one of 4096 stable process-lock shards.
pub(super) fn action_lock_path(root: &Path, action_path: &Path) -> std::path::PathBuf {
    let key = action_path
        .file_name()
        .and_then(|name| name.to_str())
        .expect("action paths end in a SHA-256 key");
    root.join("flights").join(&key[..3])
}

#[cfg(test)]
mod tests {
    use super::LocalActionCache;
    use super::LocalActionReservation;
    use super::LocalActionResult;
    use crate::digest_config::DigestConfig;
    use crate::execute::action_digest::ActionDigest;

    #[test]
    fn contended_action_yields_until_its_owner_releases_or_publishes() -> bsmr_error::Result<()> {
        let temporary = tempfile::tempdir()?;
        let cache = LocalActionCache::at(temporary.path().to_owned())?;
        let action = ActionDigest::from_content(
            b"action",
            DigestConfig::testing_default().cas_digest_config(),
        );
        let owner = cache.try_reserve_action(&action)?;
        assert!(matches!(owner, Some(LocalActionReservation::Lease(_))));
        assert!(cache.try_reserve_action(&action)?.is_none());
        drop(owner);
        let next = cache.try_reserve_action(&action)?;
        assert!(matches!(next, Some(LocalActionReservation::Lease(_))));
        cache.publish_action_result(&action, &LocalActionResult::default())?;
        drop(next);
        let hit = cache.try_reserve_action(&action)?;
        assert!(matches!(hit, Some(LocalActionReservation::Hit { .. })));
        let concurrent_hit = cache.try_reserve_action(&action)?;
        assert!(matches!(
            concurrent_hit,
            Some(LocalActionReservation::Hit { .. })
        ));
        drop(hit);
        drop(concurrent_hit);
        assert!(matches!(
            cache.try_reserve_action(&action)?,
            Some(LocalActionReservation::Hit { .. })
        ));
        Ok(())
    }
}
