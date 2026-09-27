//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Prove input snapshots cannot redirect privileged writes or acquire executable privilege bits.

#![cfg(unix)]

use std::fs;
use std::fs::File;
use std::io::{Seek, SeekFrom, Write};
use std::os::unix::fs::PermissionsExt;

use bsmr_native::archive::{Archive, Error};
use sha2::{Digest, Sha256};

/// A private fixture restores its own root's permissions before temporary cleanup.
struct Root(tempfile::TempDir);

impl Drop for Root {
    fn drop(&mut self) {
        let _ = fs::set_permissions(self.0.path(), fs::Permissions::from_mode(0o700));
    }
}

/// Build raw headers so adversarial names are not pre-rejected by tar's convenience API.
fn input(entries: &[(&str, tar::EntryType, &str)]) -> (File, String) {
    let mut tar = tar::Builder::new(Vec::new());
    for (name, kind, target) in entries {
        let mut header = tar::Header::new_gnu();
        header.as_mut_bytes()[..name.len()].copy_from_slice(name.as_bytes());
        header.set_entry_type(*kind);
        header.set_mode(0o6777);
        let payload: &[u8] = if kind.is_file() { b"input" } else { b"" };
        header.set_size(payload.len() as u64);
        if kind.is_symlink() || kind.is_hard_link() {
            header.set_link_name(target).unwrap();
        }
        header.set_cksum();
        tar.append(&header, payload).unwrap();
    }
    let bytes = tar.into_inner().unwrap();
    let digest = format!("{:x}", Sha256::digest(&bytes));
    let mut file = tempfile::tempfile().unwrap();
    file.write_all(&bytes).unwrap();
    (file, digest)
}

#[test]
fn pinned_copy_survives_source_offset_and_byte_changes() {
    let (mut source, digest) = input(&[("tool", tar::EntryType::Regular, "")]);
    let mut archive = Archive::capture(&source, &digest).unwrap();
    source.seek(SeekFrom::Start(0)).unwrap();
    source.write_all(b"changed").unwrap();
    assert!(matches!(
        Archive::capture(&source, &digest),
        Err(Error::Digest { .. })
    ));
    let root = Root(tempfile::tempdir().unwrap());
    archive.unpack(root.0.path()).unwrap();
    assert_eq!(fs::read(root.0.path().join("tool")).unwrap(), b"input");
    assert_eq!(
        fs::metadata(root.0.path().join("tool"))
            .unwrap()
            .permissions()
            .mode()
            & 0o7777,
        0o555
    );
}

#[test]
fn links_cannot_redirect_archive_writes() {
    let cases = [
        vec![("../outside", tar::EntryType::Regular, "")],
        vec![("link", tar::EntryType::Symlink, "../../outside")],
        vec![
            ("link", tar::EntryType::Symlink, "target"),
            ("link/child", tar::EntryType::Regular, ""),
        ],
        vec![("link", tar::EntryType::Link, "../outside")],
        vec![("device", tar::EntryType::Char, "")],
        vec![
            ("file", tar::EntryType::Regular, ""),
            ("file", tar::EntryType::Regular, ""),
        ],
    ];
    for entries in cases {
        let (source, digest) = input(&entries);
        let root = Root(tempfile::tempdir().unwrap());
        assert!(
            Archive::capture(&source, &digest)
                .unwrap()
                .unpack(root.0.path())
                .is_err(),
            "{entries:?}"
        );
    }
}

#[test]
fn internal_aliases_preserve_readonly_payloads() {
    let (source, digest) = input(&[
        ("file", tar::EntryType::Regular, ""),
        ("hard", tar::EntryType::Link, "file"),
        ("soft", tar::EntryType::Symlink, "file"),
    ]);
    let root = Root(tempfile::tempdir().unwrap());
    Archive::capture(&source, &digest)
        .unwrap()
        .unpack(root.0.path())
        .unwrap();
    for name in ["file", "hard", "soft"] {
        assert_eq!(fs::read(root.0.path().join(name)).unwrap(), b"input");
    }
}

#[test]
fn sparse_oversized_input_is_rejected_before_reading() {
    let source = tempfile::tempfile().unwrap();
    source.set_len((1 << 30) + 1).unwrap();
    assert!(matches!(Archive::capture(&source, ""), Err(Error::Limit)));
}
