//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Captures Git database inputs once per command, including metadata outside linked worktrees.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::Path;
use std::sync::Arc;

use allocative::Allocative;
use dice::InjectedKey;
use dice::PagableValueSerialize;
use dice::ValueSerialize;
use dupe::Dupe;
use pagable::Pagable;
use pagable::pagable_typetag;
use serde::Serialize;
use sha2::Digest;
use sha2::Sha256;

/// Immutable bytes that materialization must verify before a build script can read them.
#[derive(Debug, Eq, PartialEq, Allocative, Pagable, Serialize)]
pub struct GitFile {
    /// Credential-free local source URL, resolved before compilation.
    pub url: String,
    /// Expected digest of the complete file.
    pub sha256: String,
    /// Expected file length in bytes.
    pub size: u64,
}

/// Changes to external HEAD, index or refs must invalidate a persistent daemon's analysis.
#[derive(
    Clone,
    Dupe,
    Debug,
    Eq,
    PartialEq,
    Hash,
    Allocative,
    Pagable,
    derive_more::Display
)]
#[display("GitInputs")]
#[pagable_typetag(dice::DiceKeyDyn)]
pub struct GitInputs;

impl InjectedKey for GitInputs {
    type Value = Arc<BTreeMap<String, GitFile>>;

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }

    fn value_serialize() -> impl ValueSerialize<Value = Self::Value> {
        PagableValueSerialize::<Self::Value>::new()
    }
}

/// Preserve worktree-specific HEAD/index and shared objects/refs without exposing hooks.
pub fn capture(root: &Path) -> bsmr_error::Result<BTreeMap<String, GitFile>> {
    let mut files = BTreeMap::new();
    let marker = root.join(".git");
    if !root.join("Cargo.toml").is_file() || !marker.exists() {
        return Ok(files);
    }
    let git = if marker.is_file() {
        let pointer = fs::read_to_string(&marker)?;
        let path = pointer
            .strip_prefix("gitdir:")
            .ok_or_else(|| super::unsupported("Git", "invalid worktree pointer"))?;
        root.join(path.trim()).canonicalize()?
    } else {
        marker.canonicalize()?
    };
    let common = match fs::read_to_string(git.join("commondir")) {
        Ok(path) => git.join(path.trim()).canonicalize()?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => git.clone(),
        Err(error) => return Err(error.into()),
    };
    for name in ["HEAD", "index"] {
        insert(&mut files, &git.join(name), &format!(".git/{name}"))?;
    }
    for name in ["packed-refs", "shallow", "info/exclude"] {
        insert(&mut files, &common.join(name), &format!(".git/{name}"))?;
    }
    for entry in fs::read_dir(&git)? {
        let entry = entry?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| super::unsupported("Git", "non-UTF-8 index name"))?;
        if name.starts_with("sharedindex.") {
            insert(&mut files, &entry.path(), &format!(".git/{name}"))?;
        }
    }
    directory(&mut files, &common.join("refs"), ".git/refs")?;
    let mut pending = vec![common.join("objects")];
    let mut visited = BTreeSet::new();
    while let Some(objects) = pending.pop() {
        let objects = objects.canonicalize()?;
        if !visited.insert(objects.clone()) {
            continue;
        }
        match fs::read_to_string(objects.join("info/alternates")) {
            Ok(paths) => pending.extend(paths.lines().map(|path| objects.join(path))),
            Err(error) if error.kind() == io::ErrorKind::NotFound => (),
            Err(error) => return Err(error.into()),
        }
        directory(&mut files, &objects, ".git/objects")?;
    }
    Ok(files)
}

/// Keep native inputs file-granular so unchanged object packs are shared by every script.
fn directory(
    files: &mut BTreeMap<String, GitFile>,
    root: &Path,
    prefix: &str,
) -> bsmr_error::Result<()> {
    let entries = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    for entry in entries {
        let entry = entry?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| super::unsupported("Git", "non-UTF-8 database path"))?;
        if (prefix == ".git/objects/info" && name == "alternates") || name.ends_with(".lock") {
            continue;
        }
        let path = format!("{prefix}/{name}");
        if entry.file_type()?.is_dir() {
            directory(files, &entry.path(), &path)?;
        } else {
            insert(files, &entry.path(), &path)?;
        }
    }
    Ok(())
}

/// Hash the exact bytes rather than trusting timestamps from a shared Git directory.
fn insert(
    files: &mut BTreeMap<String, GitFile>,
    source: &Path,
    name: &str,
) -> bsmr_error::Result<()> {
    let mut file = match fs::File::open(source) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.into()),
    };
    let mut hash = Sha256::new();
    let size = io::copy(&mut file, &mut hash)?;
    let captured = GitFile {
        url: url::Url::from_file_path(source)
            .map_err(|()| super::unsupported("Git", "database path must be absolute"))?
            .into(),
        sha256: format!("{:x}", hash.finalize()),
        size,
    };
    if let Some(previous) = files.get(name) {
        if previous.sha256 != captured.sha256 {
            return Err(super::unsupported(name, "conflicting Git object stores").into());
        }
    } else {
        files.insert(name.to_owned(), captured);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Worktrees share objects and refs while retaining their own HEAD and index.
    #[test]
    fn invariant_worktree_inputs_preserve_metadata_ownership() -> bsmr_error::Result<()> {
        let root = tempfile::tempdir()?;
        let source = root.path().join("source");
        let common = root.path().join("git");
        let worktree = common.join("worktrees/task");
        fs::create_dir_all(&source)?;
        fs::create_dir_all(&worktree)?;
        fs::create_dir_all(common.join("refs/heads"))?;
        fs::create_dir_all(common.join("objects"))?;
        fs::write(source.join("Cargo.toml"), "[workspace]\n")?;
        fs::write(source.join(".git"), "gitdir: ../git/worktrees/task\n")?;
        fs::write(worktree.join("commondir"), "../..\n")?;
        fs::write(worktree.join("HEAD"), "ref: refs/heads/task\n")?;
        fs::write(worktree.join("index"), "worktree index")?;
        fs::write(common.join("HEAD"), "wrong HEAD")?;
        fs::write(common.join("index"), "wrong index")?;
        fs::write(common.join("refs/heads/task"), "old revision")?;
        fs::write(common.join("refs/heads/alternates"), "other revision")?;
        fs::write(
            common.join("config"),
            "credentials and hooks must not enter the action",
        )?;
        let first = capture(&source)?;
        assert_eq!(first, capture(&source)?);
        assert!(!first.contains_key(".git/config"));
        assert!(first.contains_key(".git/refs/heads/alternates"));
        assert!(first[".git/index"].url.ends_with("worktrees/task/index"));
        fs::write(common.join("refs/heads/task"), "new revision")?;
        let changed = capture(&source)?;
        assert_ne!(first, changed);
        assert_eq!(first[".git/index"], changed[".git/index"]);
        fs::write(worktree.join("index"), "new worktree index")?;
        assert_ne!(changed, capture(&source)?);
        Ok(())
    }
}
