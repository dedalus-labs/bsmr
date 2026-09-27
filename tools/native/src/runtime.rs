//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Retain a verified runtime snapshot and share its immutable files across action roots.

use std::fs;
use std::io::{self, Read};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

use nix::sys::stat::{Mode, SFlag, mknod};
use nix::unistd::Uid;
use sha2::{Digest, Sha256};
use thiserror::Error;

/// A private snapshot whose files remain root-owned and read-only for every action UID.
pub struct Runtime {
    /// Retained until the runtime is replaced. Job roots retain their own hard links.
    snapshot: tempfile::TempDir,
    /// Identity of the exact normalized files and trusted launcher bytes.
    digest: String,
}

/// Runtime failures are configuration errors and never select a less isolated backend.
#[derive(Debug, Error)]
pub enum Error {
    #[error("native runtime preparation requires real and effective root")]
    Root,
    #[error("native runtime path must be root-owned and unwritable by others: {0:?}")]
    Ownership(PathBuf),
    #[error("native runtime contains a reserved name or unsupported entry: {0:?}")]
    Entry(PathBuf),
    #[error("native runtime I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("native runtime device setup failed: {0}")]
    Kernel(#[from] nix::errno::Errno),
}

impl Runtime {
    /// Capture administrator-configured files once, without retaining mutable source paths.
    ///
    /// `state` is a protected directory on the filesystem used for action roots.
    /// `seed` contains runtime files only. `launcher` is the installed trusted
    /// binary. The installer owns these paths and replaces them atomically.
    pub fn capture(seed: &Path, state: &Path, launcher: &Path) -> Result<Self, Error> {
        if !Uid::current().is_root() || !Uid::effective().is_root() {
            return Err(Error::Root);
        }
        for path in [seed, state, launcher] {
            protected(path)?;
        }
        for name in ["workspace", ".bsmr", "dev", "tmp"] {
            match fs::symlink_metadata(seed.join(name)) {
                Ok(_) => return Err(Error::Entry(seed.join(name))),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        let snapshot = tempfile::Builder::new()
            .prefix("runtime-")
            .tempdir_in(state)?;
        let mut hash = Sha256::new();
        hash.update(b"bsmr-native-runtime-v1\0");
        hash.update(std::env::consts::ARCH.as_bytes());
        hash_file(launcher, &mut hash)?;
        capture(seed, snapshot.path(), Path::new(""), &mut hash)?;
        Ok(Self {
            snapshot,
            digest: format!("{:x}", hash.finalize()),
        })
    }

    /// Return the identity that must enter the action key and request before execution.
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }

    /// Share immutable file inodes while giving the job private directories and scratch space.
    pub fn instantiate(&self) -> Result<tempfile::TempDir, Error> {
        let state = self
            .snapshot
            .path()
            .parent()
            .expect("snapshot has its protected parent");
        protected(state)?;
        let root = tempfile::Builder::new()
            .prefix("action-")
            .tempdir_in(state)?;
        link(self.snapshot.path(), root.path())?;
        fs::create_dir(root.path().join("tmp"))?;
        fs::set_permissions(root.path().join("tmp"), fs::Permissions::from_mode(0o1777))?;
        fs::create_dir(root.path().join("dev"))?;
        let device = fs::metadata("/dev/null")?.rdev().try_into().map_err(|_| {
            io::Error::other("native null device identifier exceeds the platform's device type")
        })?;
        mknod(
            &root.path().join("dev/null"),
            SFlag::S_IFCHR,
            Mode::from_bits_truncate(0o666),
            device,
        )?;
        fs::set_permissions(
            root.path().join("dev/null"),
            fs::Permissions::from_mode(0o666),
        )?;
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o755))?;
        Ok(root)
    }
}

/// Require protected ownership before privileged path traversal begins.
fn protected(path: &Path) -> Result<(), Error> {
    if !path.is_absolute() {
        return Err(Error::Ownership(path.to_owned()));
    }
    for ancestor in path.ancestors() {
        let metadata = fs::symlink_metadata(ancestor)?;
        if metadata.uid() != 0 || metadata.mode() & 0o022 != 0 || metadata.is_symlink() {
            return Err(Error::Ownership(ancestor.to_owned()));
        }
    }
    Ok(())
}

/// Copy before hashing so the identity always describes the retained bytes.
fn capture(
    source: &Path,
    destination: &Path,
    relative: &Path,
    hash: &mut Sha256,
) -> Result<(), Error> {
    let mut entries = fs::read_dir(source)?.collect::<io::Result<Vec<_>>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let metadata = entry.metadata()?;
        let source = entry.path();
        let destination = destination.join(entry.file_name());
        let relative = relative.join(entry.file_name());
        // The snapshot has one writer. Reject case-folded aliases before any copy
        // could replace bytes that have already entered the runtime identity.
        match fs::symlink_metadata(&destination) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Ok(_) => return Err(Error::Entry(relative)),
            Err(error) => return Err(error.into()),
        }
        if metadata.uid() != 0 || (!metadata.is_symlink() && metadata.mode() & 0o022 != 0) {
            return Err(Error::Ownership(source));
        }
        let name = relative.as_os_str().as_bytes();
        hash.update((name.len() as u64).to_le_bytes());
        hash.update(name);
        if metadata.is_dir() {
            hash.update(b"d");
            fs::create_dir(&destination)?;
            capture(&source, &destination, &relative, hash)?;
            fs::set_permissions(destination, fs::Permissions::from_mode(0o555))?;
        } else if metadata.is_file() {
            let mode = if metadata.mode() & 0o111 == 0 {
                0o444_u32
            } else {
                0o555_u32
            };
            fs::copy(source, &destination)?;
            fs::set_permissions(&destination, fs::Permissions::from_mode(mode))?;
            hash.update(b"f");
            hash.update(mode.to_le_bytes());
            hash_file(&destination, hash)?;
        } else if metadata.is_symlink() {
            let target = fs::read_link(source)?;
            hash.update(b"s");
            let bytes = target.as_os_str().as_bytes();
            hash.update((bytes.len() as u64).to_le_bytes());
            hash.update(bytes);
            std::os::unix::fs::symlink(target, destination)?;
        } else {
            return Err(Error::Entry(source));
        }
    }
    Ok(())
}

/// Length-prefix every file so different directory trees cannot share an ambiguous encoding.
fn hash_file(path: &Path, hash: &mut Sha256) -> io::Result<()> {
    let mut file = fs::File::open(path)?;
    hash.update(file.metadata()?.len().to_le_bytes());
    let mut bytes = [0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut bytes)?;
        if count == 0 {
            return Ok(());
        }
        hash.update(&bytes[..count]);
    }
}

/// Job UIDs cannot mutate these shared root-owned files, including through another hard link.
fn link(source: &Path, destination: &Path) -> Result<(), Error> {
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let destination = destination.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            fs::create_dir(&destination)?;
            link(&entry.path(), &destination)?;
            fs::set_permissions(destination, fs::Permissions::from_mode(0o555))?;
        } else if entry.file_type()?.is_symlink() {
            std::os::unix::fs::symlink(fs::read_link(entry.path())?, destination)?;
        } else {
            fs::hard_link(entry.path(), destination)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinary_users_cannot_prepare_privileged_runtime_snapshots() {
        if Uid::current().is_root() {
            return;
        }
        assert!(matches!(
            Runtime::capture(Path::new("/"), Path::new("/"), Path::new("/bin/sh")),
            Err(Error::Root)
        ));
    }
}
