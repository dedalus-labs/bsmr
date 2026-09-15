//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Coordinates cache misses without holding an I/O permit while another process runs.

use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::fs::TryLockError;

use super::LocalActionCache;
use super::LocalActionResult;
use super::io_error;
use crate::execute::action_digest::ActionDigest;

/// Returns a published result or ownership of the missing action.
pub enum LocalActionReservation {
    Hit(LocalActionResult),
    Lease(LocalActionLease),
}

/// Closing this file releases the process lock, including on execution failure.
pub struct LocalActionLease {
    _file: File,
}

impl LocalActionCache {
    /// Returns `None` immediately when another process owns the action's lock.
    pub fn try_reserve_action(
        &self,
        action: &ActionDigest,
    ) -> bsmr_error::Result<Option<LocalActionReservation>> {
        if let Some(result) = self.action_result(action)? {
            return Ok(Some(LocalActionReservation::Hit(result)));
        }
        let directory = self.root.join("flights");
        fs::create_dir_all(&directory)
            .map_err(|error| io_error("create action lock directory", &directory, error))?;
        // Three hexadecimal digits bound persistent lock files to 4096 shards.
        let path = directory.join(&action.raw_digest().to_string()[..3]);
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
        let lease = LocalActionLease { _file: file };
        Ok(Some(match self.action_result(action)? {
            Some(result) => LocalActionReservation::Hit(result),
            None => LocalActionReservation::Lease(lease),
        }))
    }
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
        assert!(matches!(
            cache.try_reserve_action(&action)?,
            Some(LocalActionReservation::Hit(_))
        ));
        Ok(())
    }
}
