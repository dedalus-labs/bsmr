//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Snapshot caller-owned archives before expanding them into a private action root.

use std::collections::BTreeSet;
use std::fs;
use std::fs::File;
use std::io::{self, Seek, SeekFrom, Write};
use std::os::unix::fs::{FileExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};

use sha2::{Digest, Sha256};
use thiserror::Error;

/// Bound both the incoming archive and its expanded payload to one GiB.
const MAX_BYTES: u64 = 1024 * 1024 * 1024;
/// Bound metadata independently of the byte payload.
const MAX_ENTRIES: usize = 200_000;

/// Failed input validation never yields an executable action root.
#[derive(Debug, Error)]
pub enum Error {
    #[error("native input exceeds the archive or entry limit")]
    Limit,
    #[error("native input digest is {actual}, expected {expected}")]
    Digest { expected: String, actual: String },
    #[error("native archive entry is invalid, duplicated or unsupported: {0:?}")]
    Entry(PathBuf),
    #[error("native archive destination must be empty")]
    NotEmpty,
    #[error("native archive I/O failed: {0}")]
    Io(#[from] io::Error),
}

/// Private immutable archive bytes. No descriptor is returned to the caller.
pub struct Archive(File);

impl Archive {
    /// Copy the opened input with bounded memory and verify the exact copied bytes.
    ///
    /// Positional reads ignore changes to the sender's shared descriptor offset.
    /// The private copy also prevents later sender writes from changing extraction.
    pub fn capture(source: &File, expected: &str) -> Result<Self, Error> {
        let size = source.metadata()?.len();
        if size > MAX_BYTES {
            return Err(Error::Limit);
        }
        let mut private = tempfile::tempfile()?;
        let mut hash = Sha256::new();
        let mut offset = 0;
        let mut buffer = [0_u8; 64 * 1024];
        while offset < size {
            let count = usize::try_from((size - offset).min(buffer.len() as u64)).unwrap();
            let read = source.read_at(&mut buffer[..count], offset)?;
            if read == 0 {
                return Err(io::Error::from(io::ErrorKind::UnexpectedEof).into());
            }
            private.write_all(&buffer[..read])?;
            hash.update(&buffer[..read]);
            offset += read as u64;
        }
        let actual = format!("{:x}", hash.finalize());
        if actual != expected {
            return Err(Error::Digest {
                expected: expected.into(),
                actual,
            });
        }
        Ok(Self(private))
    }

    /// Expand only into a new supervisor-owned directory, before starting any workload.
    ///
    /// Links are installed last, so no archive write can traverse a link. Every
    /// accepted entry remains inside the root and loses write and privilege bits.
    /// The supervisor owns cleanup and must keep this tree private through validation.
    pub fn unpack(&mut self, root: &Path) -> Result<(), Error> {
        if fs::read_dir(root)?.next().is_some() {
            return Err(Error::NotEmpty);
        }
        self.0.seek(SeekFrom::Start(0))?;
        let mut paths = BTreeSet::new();
        let mut files = BTreeSet::new();
        let mut links = Vec::new();
        let mut bytes = 0_u64;
        for entry in tar::Archive::new(&mut self.0).entries()? {
            let mut entry = entry?;
            let path = entry.path()?.into_owned();
            if !normal(&path) || !paths.insert(path.clone()) {
                return Err(Error::Entry(path));
            }
            bytes = bytes.checked_add(entry.size()).ok_or(Error::Limit)?;
            if bytes > MAX_BYTES || paths.len() > MAX_ENTRIES {
                return Err(Error::Limit);
            }
            let destination = root.join(&path);
            fs::create_dir_all(
                destination
                    .parent()
                    .ok_or_else(|| Error::Entry(path.clone()))?,
            )?;
            let kind = entry.header().entry_type();
            if kind.is_dir() {
                fs::create_dir_all(&destination)?;
            } else if kind.is_file() {
                let mode = if entry.header().mode()? & 0o111 == 0 {
                    0o444
                } else {
                    0o555
                };
                let mut output = File::options()
                    .write(true)
                    .create_new(true)
                    .open(&destination)?;
                let expected = entry.size();
                if io::copy(&mut entry, &mut output)? != expected {
                    return Err(io::Error::from(io::ErrorKind::UnexpectedEof).into());
                }
                output.set_permissions(fs::Permissions::from_mode(mode))?;
                files.insert(path);
            } else if kind.is_hard_link() || kind.is_symlink() {
                let target = entry
                    .link_name()?
                    .ok_or_else(|| Error::Entry(path.clone()))?
                    .into_owned();
                if entry.size() != 0 || (kind.is_hard_link() && !files.contains(&target)) {
                    return Err(Error::Entry(path));
                }
                if kind.is_symlink() && !contained(&path, &target) {
                    return Err(Error::Entry(path));
                }
                links.push((path, target, kind));
            } else {
                return Err(Error::Entry(path));
            }
        }
        for (path, target, kind) in links {
            if kind.is_hard_link() {
                fs::hard_link(root.join(target), root.join(path))?;
            } else {
                std::os::unix::fs::symlink(target, root.join(path))?;
            }
        }
        readonly(root)
    }
}

/// Accept a nonempty relative path with no dot components or platform escapes.
pub(crate) fn normal(path: &Path) -> bool {
    !path.as_os_str().is_empty()
        && path
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
}

/// Resolve a link lexically without following any filesystem object.
fn contained(path: &Path, target: &Path) -> bool {
    let mut depth = path.components().count() - 1;
    for part in target.components() {
        match part {
            Component::Normal(_) => depth += 1,
            Component::CurDir => {}
            Component::ParentDir if depth > 0 => depth -= 1,
            _ => return false,
        }
    }
    !target.as_os_str().is_empty()
}

/// Freeze implicit parents too. The root is private and has no running writers.
fn readonly(root: &Path) -> Result<(), Error> {
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            readonly(&entry.path())?;
        }
    }
    fs::set_permissions(root, fs::Permissions::from_mode(0o555))?;
    Ok(())
}
