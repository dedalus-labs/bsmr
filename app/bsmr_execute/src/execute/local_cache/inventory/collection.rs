//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Collects unreachable blobs and oldest action closures under an exact byte budget.

use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use super::CacheEntry;
use super::cache_entries;
use super::read_action_result;
use super::temporary_entries;
use crate::digest_config::DigestConfig;
use crate::execute::local_cache::LocalActionCache;
use crate::execute::local_cache::LocalActionPin;
use crate::execute::local_cache::io_error;

/// Reports the exact entries selected or removed by one collection pass.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LocalCacheCollection {
    pub removed_action_results: u64,
    pub removed_blobs: u64,
    pub removed_bytes: u64,
    pub remaining_bytes: u64,
    pub removed_temporary_files: u64,
}

struct ActionRecord {
    entry: CacheEntry,
    paths: HashSet<PathBuf>,
    complete: bool,
    pin: Option<Arc<LocalActionPin>>,
}

impl ActionRecord {
    /// Reports an action pinned by another process or deferred materialization.
    fn is_pinned(&self) -> bool {
        self.pin.is_none()
    }
}

impl LocalActionCache {
    /// Removes unreachable data and then evicts oldest actions to meet `max_bytes`.
    pub fn collect(
        &self,
        max_bytes: u64,
        dry_run: bool,
        digest_config: DigestConfig,
    ) -> bsmr_error::Result<LocalCacheCollection> {
        let _lock = self.exclusive_lock()?;
        let actions = cache_entries(&self.root.join("ac"))?;
        let blobs = cache_entries(&self.root.join("cas"))?;
        let temporary = temporary_entries(&self.root.join("ac"))?
            .into_iter()
            .chain(temporary_entries(&self.root.join("cas"))?)
            .collect::<Vec<_>>();
        let blob_bytes = blobs
            .iter()
            .map(|entry| (entry.path.clone(), entry.bytes))
            .collect::<HashMap<_, _>>();
        let mut records = Vec::with_capacity(actions.len());
        let mut references = HashMap::<PathBuf, u64>::new();
        for entry in actions {
            let pin = self.try_collect_action_path(&entry.path)?;
            let closure = self.result_closure(&read_action_result(&entry.path)?, digest_config)?;
            for path in &closure.paths {
                *references.entry(path.clone()).or_default() += 1;
            }
            records.push(ActionRecord {
                entry,
                paths: closure.paths,
                complete: closure.complete,
                pin,
            });
        }

        records.sort_by_key(|record| (record.entry.modified, record.entry.path.clone()));
        let action_bytes = records
            .iter()
            .map(|record| (record.entry.path.clone(), record.entry.bytes))
            .collect::<HashMap<_, _>>();
        let mut removed_actions = HashSet::new();
        let mut removed_blobs = blob_bytes
            .keys()
            .filter(|path| !references.contains_key(*path))
            .cloned()
            .collect::<HashSet<_>>();
        let mut remaining_bytes = records.iter().map(|record| record.entry.bytes).sum::<u64>()
            + blob_bytes.values().sum::<u64>()
            - removed_blobs
                .iter()
                .filter_map(|path| blob_bytes.get(path))
                .sum::<u64>();

        for record in records
            .iter()
            .filter(|record| !record.complete && !record.is_pinned())
        {
            evict_action(
                record,
                &mut references,
                &blob_bytes,
                &mut removed_actions,
                &mut removed_blobs,
                &mut remaining_bytes,
            );
        }
        for record in records
            .iter()
            .filter(|record| record.complete && !record.is_pinned())
        {
            if remaining_bytes <= max_bytes {
                break;
            }
            evict_action(
                record,
                &mut references,
                &blob_bytes,
                &mut removed_actions,
                &mut removed_blobs,
                &mut remaining_bytes,
            );
        }

        if !dry_run {
            remove_entries(&removed_actions, "remove cache action")?;
            remove_entries(&removed_blobs, "remove cache blob")?;
            remove_entries(
                &temporary.iter().map(|entry| entry.path.clone()).collect(),
                "remove interrupted cache write",
            )?;
        }
        let removed_bytes = removed_actions
            .iter()
            .filter_map(|path| action_bytes.get(path))
            .sum::<u64>()
            + removed_blobs
                .iter()
                .filter_map(|path| blob_bytes.get(path))
                .sum::<u64>()
            + temporary.iter().map(|entry| entry.bytes).sum::<u64>();
        Ok(LocalCacheCollection {
            removed_action_results: removed_actions.len() as u64,
            removed_blobs: removed_blobs.len() as u64,
            removed_bytes,
            remaining_bytes,
            removed_temporary_files: temporary.len() as u64,
        })
    }
}

/// Selects one action root before decrementing its reachable blob references.
fn evict_action(
    record: &ActionRecord,
    references: &mut HashMap<PathBuf, u64>,
    blob_bytes: &HashMap<PathBuf, u64>,
    removed_actions: &mut HashSet<PathBuf>,
    removed_blobs: &mut HashSet<PathBuf>,
    remaining_bytes: &mut u64,
) {
    if !removed_actions.insert(record.entry.path.clone()) {
        return;
    }
    *remaining_bytes = remaining_bytes.saturating_sub(record.entry.bytes);
    for path in &record.paths {
        let Some(count) = references.get_mut(path) else {
            continue;
        };
        *count -= 1;
        if *count == 0
            && let Some(bytes) = blob_bytes.get(path)
            && removed_blobs.insert(path.clone())
        {
            *remaining_bytes = remaining_bytes.saturating_sub(*bytes);
        }
    }
}

/// Deletes exact immutable entry paths after the collection plan is complete.
fn remove_entries(paths: &HashSet<PathBuf>, operation: &'static str) -> bsmr_error::Result<()> {
    for path in paths {
        fs::remove_file(path).map_err(|error| io_error(operation, path, error))?;
    }
    Ok(())
}
