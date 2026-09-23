//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Inventories local action results and their reachable content-addressed blobs.

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;

use prost::Message;

use super::LocalActionCache;
use super::LocalActionResult;
use super::LocalCacheError;
use super::LocalDigest;
use super::io_error;
use super::read_regular_file;
use super::validate_blob_path;
use crate::digest_config::DigestConfig;

mod collection;

pub use collection::LocalCacheCollection;

/// Counts the action roots and CAS blobs visible to a local cache scan.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct LocalCacheInventory {
    pub action_results: u64,
    pub complete_actions: u64,
    pub incomplete_actions: u64,
    pub blobs: u64,
    pub reachable_blobs: u64,
    pub orphan_blobs: u64,
    pub action_bytes: u64,
    pub blob_bytes: u64,
    pub temporary_files: u64,
    pub temporary_bytes: u64,
}

struct CacheEntry {
    path: PathBuf,
    bytes: u64,
    modified: SystemTime,
}

struct ResultClosure {
    paths: HashSet<PathBuf>,
    complete: bool,
}

impl LocalActionCache {
    /// Scans the cache without writing or deleting entries.
    pub fn inventory(
        &self,
        digest_config: DigestConfig,
    ) -> bsmr_error::Result<LocalCacheInventory> {
        let _lock = self.shared_lock()?;
        self.inventory_unlocked(digest_config)
    }

    /// Scans the cache while the caller holds the shared or exclusive lock.
    fn inventory_unlocked(
        &self,
        digest_config: DigestConfig,
    ) -> bsmr_error::Result<LocalCacheInventory> {
        let actions = cache_entries(&self.root.join("ac"))?;
        let blobs = cache_entries(&self.root.join("cas"))?;
        let temporary = temporary_entries(&self.root.join("ac"))?
            .into_iter()
            .chain(temporary_entries(&self.root.join("cas"))?)
            .collect::<Vec<_>>();
        let blob_paths = blobs
            .iter()
            .map(|entry| entry.path.clone())
            .collect::<HashSet<_>>();
        let mut reachable = HashSet::new();
        let mut complete_actions = 0;

        for action in &actions {
            let result = read_action_result(&action.path)?;
            let closure = self.result_closure(&result, digest_config)?;
            reachable.extend(closure.paths);
            complete_actions += u64::from(closure.complete);
        }

        let reachable_blobs = blob_paths.intersection(&reachable).count() as u64;
        Ok(LocalCacheInventory {
            action_results: actions.len() as u64,
            complete_actions,
            incomplete_actions: actions.len() as u64 - complete_actions,
            blobs: blobs.len() as u64,
            reachable_blobs,
            orphan_blobs: blobs.len() as u64 - reachable_blobs,
            action_bytes: actions.iter().map(|entry| entry.bytes).sum(),
            blob_bytes: blobs.iter().map(|entry| entry.bytes).sum(),
            temporary_files: temporary.len() as u64,
            temporary_bytes: temporary.iter().map(|entry| entry.bytes).sum(),
        })
    }

    /// Resolves every blob named by one action result and its output trees.
    fn result_closure(
        &self,
        result: &LocalActionResult,
        digest_config: DigestConfig,
    ) -> bsmr_error::Result<ResultClosure> {
        let mut closure = ResultClosure {
            paths: HashSet::new(),
            complete: true,
        };
        for digest in result
            .output_files
            .iter()
            .map(|file| &file.digest)
            .chain(result.stdout.iter())
            .chain(result.stderr.iter())
        {
            self.add_blob(digest, digest_config, &mut closure)?;
        }
        for directory in &result.output_directories {
            self.add_tree(&directory.tree_digest, digest_config, &mut closure)?;
        }
        Ok(closure)
    }

    /// Adds one direct blob reference and records whether it is present.
    fn add_blob(
        &self,
        digest: &LocalDigest,
        digest_config: DigestConfig,
        closure: &mut ResultClosure,
    ) -> bsmr_error::Result<()> {
        digest.to_file_digest(digest_config)?;
        let path = self.blob_path(digest);
        closure.paths.insert(path.clone());
        closure.complete &= validate_blob_path(&path, digest.size)?;
        Ok(())
    }

    /// Adds one RE Tree and every file digest reachable from it.
    fn add_tree(
        &self,
        digest: &LocalDigest,
        digest_config: DigestConfig,
        closure: &mut ResultClosure,
    ) -> bsmr_error::Result<()> {
        self.add_blob(digest, digest_config, closure)?;
        let Some(tree_bytes) = self.read_blob_unlocked(digest, digest_config)? else {
            return Ok(());
        };
        let tree = remote_execution::Tree::decode(tree_bytes.as_slice()).map_err(|source| {
            LocalCacheError::DecodeTree {
                path: self.blob_path(digest),
                source,
            }
        })?;
        if tree.root.is_none() {
            return Err(LocalCacheError::MissingTreeRoot.into());
        }
        for file in tree
            .root
            .iter()
            .chain(tree.children.iter())
            .flat_map(|directory| &directory.files)
        {
            let file = file
                .digest
                .as_ref()
                .ok_or(LocalCacheError::MissingFileDigest)?;
            self.add_blob(
                &LocalDigest {
                    algorithm: digest.algorithm.clone(),
                    hash: file.hash.clone(),
                    size: file.size_bytes,
                },
                digest_config,
                closure,
            )?;
        }
        Ok(())
    }
}

/// Lists immutable cache entries while excluding interrupted temporary writes.
fn cache_entries(root: &Path) -> bsmr_error::Result<Vec<CacheEntry>> {
    matching_entries(root, is_cache_key)
}

/// Lists interrupted writes that cannot be active while collection holds its lock.
fn temporary_entries(root: &Path) -> bsmr_error::Result<Vec<CacheEntry>> {
    matching_entries(root, is_temporary_key)
}

fn matching_entries(
    root: &Path,
    matches: impl Fn(&Path) -> bool,
) -> bsmr_error::Result<Vec<CacheEntry>> {
    let prefixes = match fs::read_dir(root) {
        Ok(prefixes) => prefixes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(io_error("list cache", root, error)),
    };
    let mut entries = Vec::new();
    for prefix in prefixes {
        let prefix = prefix.map_err(|error| io_error("list cache", root, error))?;
        if !prefix
            .file_type()
            .map_err(|error| io_error("inspect cache", prefix.path(), error))?
            .is_dir()
        {
            continue;
        }
        let children = fs::read_dir(prefix.path())
            .map_err(|error| io_error("list cache prefix", prefix.path(), error))?;
        for child in children {
            let child =
                child.map_err(|error| io_error("list cache prefix", prefix.path(), error))?;
            let path = child.path();
            if !matches(&path) {
                continue;
            }
            let metadata = fs::symlink_metadata(&path)
                .map_err(|error| io_error("inspect cache entry", &path, error))?;
            if !metadata.is_file() {
                return Err(LocalCacheError::NotFile(path).into());
            }
            entries.push(CacheEntry {
                path,
                bytes: metadata.len(),
                modified: metadata
                    .modified()
                    .map_err(|error| io_error("read cache entry timestamp", child.path(), error))?,
            });
        }
    }
    entries.sort_by(|left, right| left.path.cmp(&right.path));
    Ok(entries)
}

/// Recognizes temporary files created beside one immutable cache key.
fn is_temporary_key(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.split_once(".tmp."))
        .is_some_and(|(key, suffix)| {
            let mut parts = suffix.split('.');
            let pid = parts.next();
            let counter = parts.next();
            key.len() == 64
                && key
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
                && pid.is_some_and(|value| {
                    is_canonical_decimal(value, false) && value.parse::<u32>().is_ok()
                })
                && counter.is_some_and(|value| {
                    is_canonical_decimal(value, true) && value.parse::<u64>().is_ok()
                })
                && parts.next().is_none()
        })
}

/// Accepts the decimal spelling emitted by unsigned integer formatting.
fn is_canonical_decimal(value: &str, allow_zero: bool) -> bool {
    match value.as_bytes() {
        b"0" => allow_zero,
        [first, rest @ ..] => (b'1'..=b'9').contains(first) && rest.iter().all(u8::is_ascii_digit),
        [] => false,
    }
}

/// Decodes one durable action root without accepting a missing file as a miss.
fn read_action_result(path: &Path) -> bsmr_error::Result<LocalActionResult> {
    let bytes = read_regular_file(path)?.ok_or_else(|| {
        io_error(
            "read inventory action",
            path,
            io::Error::from(io::ErrorKind::NotFound),
        )
    })?;
    serde_json::from_slice(&bytes).map_err(|source| {
        LocalCacheError::DecodeAction {
            path: path.to_owned(),
            source,
        }
        .into()
    })
}

/// Recognizes the SHA-256 filenames used for AC and CAS entry placement.
fn is_cache_key(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.len() == 64 && name.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::is_temporary_key;

    #[test]
    fn temporary_key_requires_canonical_writer_spelling() {
        let key = "a".repeat(64);
        assert!(is_temporary_key(Path::new(&format!("{key}.tmp.1.0"))));
        for suffix in ["+1.+0", "01.0", "0.0", "1.00", "1.0.keep"] {
            assert!(!is_temporary_key(Path::new(&format!("{key}.tmp.{suffix}"))));
        }
        assert!(!is_temporary_key(Path::new(&format!(
            "{}.tmp.1.0",
            key.to_ascii_uppercase()
        ))));
    }
}
