//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies runtime snapshots preserve pinned bytes and reject invalid archive inputs.

use super::*;

const LAUNCHER_DIGEST: &str = "ec9a6e9fe278eb1a471fbab6f40367d8548078b651d9c71581c57c2a6ca379e0";

/// Write a minimal pinned runtime with one executable or symlink archive member.
fn manifest(directory: &Path, symlink: bool) -> bsmr_error::Result<PathBuf> {
    fs::write(directory.join("bwrap"), b"launcher")?;
    let mut archive = tar::Builder::new(File::create(directory.join("rootfs.tar"))?);
    let mut header = tar::Header::new_gnu();
    header.set_mode(0o755);
    if symlink {
        header.set_entry_type(tar::EntryType::Symlink);
        header.set_size(0);
        header.set_link_name("../../outside")?;
        header.set_cksum();
        archive.append_data(&mut header, "bin/tool", std::io::empty())?;
    } else {
        header.set_size(4);
        header.set_cksum();
        archive.append_data(&mut header, "bin/tool", &b"tool"[..])?;
    }
    archive.finish()?;
    let mut manifest = BTreeMap::new();
    for (name, file) in [("bubblewrap", "bwrap"), ("rootfs", "rootfs.tar")] {
        let mut digest = CasDigestData::digester_for_algorithm(DigestAlgorithm::Sha256);
        digest.update(&fs::read(directory.join(file))?);
        manifest.insert(
            name,
            BundleArtifact {
                path: file.into(),
                sha256: digest.finalize().raw_digest().to_string(),
            },
        );
    }
    let path = directory.join("runtime.json");
    fs::write(&path, serde_json::to_vec(&manifest)?)?;
    Ok(path)
}

#[test]
fn runtime_identity_and_bytes_survive_relocation_and_source_changes() -> bsmr_error::Result<()> {
    let first = tempfile::tempdir()?;
    let second = tempfile::tempdir()?;
    let loaded = Runtime::load(&manifest(first.path(), false)?, LAUNCHER_DIGEST, None)?;
    let relocated = Runtime::load(&manifest(second.path(), false)?, LAUNCHER_DIGEST, None)?;
    assert_eq!(loaded.digest(), relocated.digest());
    fs::write(first.path().join("bwrap"), b"changed")?;
    fs::write(first.path().join("rootfs.tar"), b"changed")?;
    assert_eq!(fs::read(loaded.launcher())?, b"launcher");
    assert_eq!(fs::read(loaded.root().join("bin/tool"))?, b"tool");
    assert!(Runtime::load(&first.path().join("runtime.json"), LAUNCHER_DIGEST, None).is_err());
    Ok(())
}

#[test]
fn pinned_archives_cannot_redirect_extraction_through_symlinks() -> bsmr_error::Result<()> {
    let directory = tempfile::tempdir()?;
    let error =
        Runtime::load(&manifest(directory.path(), true)?, LAUNCHER_DIGEST, None).unwrap_err();
    assert!(error.to_string().contains("unique regular file, directory"));
    Ok(())
}

#[test]
fn every_runtime_artifact_must_match_its_pin() -> bsmr_error::Result<()> {
    for file in ["bwrap", "rootfs.tar"] {
        let directory = tempfile::tempdir()?;
        let path = manifest(directory.path(), false)?;
        fs::write(directory.path().join(file), b"corrupted")?;
        assert!(
            Runtime::load(&path, LAUNCHER_DIGEST, None)
                .unwrap_err()
                .to_string()
                .contains("has digest")
        );
    }
    Ok(())
}

/// A warm runtime rechecks its inputs without creating another extracted tree.
#[test]
fn invariant_runtime_reuse_revalidates_current_bytes() -> bsmr_error::Result<()> {
    for file in ["bwrap", "rootfs.tar"] {
        let directory = tempfile::tempdir()?;
        let path = manifest(directory.path(), false)?;
        let original = Runtime::load(&path, LAUNCHER_DIGEST, None)?;
        let reused = Runtime::load(&path, LAUNCHER_DIGEST, Some(&original))?;
        assert!(Arc::ptr_eq(&original, &reused));
        fs::write(directory.path().join(file), b"corrupted")?;
        assert!(Runtime::load(&path, LAUNCHER_DIGEST, Some(&original)).is_err());
        assert_eq!(fs::read(reused.root().join("bin/tool"))?, b"tool");
    }
    Ok(())
}

/// New pins cannot replace the bytes held by an earlier command.
#[test]
fn invariant_runtime_replacement_preserves_active_snapshot() -> bsmr_error::Result<()> {
    let directory = tempfile::tempdir()?;
    let path = manifest(directory.path(), false)?;
    let original = Runtime::load(&path, LAUNCHER_DIGEST, None)?;
    let mut pins: BTreeMap<String, BundleArtifact> = serde_json::from_slice(&fs::read(&path)?)?;
    let mut archive = tar::Builder::new(File::create(directory.path().join("rootfs.tar"))?);
    let mut header = tar::Header::new_gnu();
    header.set_mode(0o755);
    header.set_size(3);
    header.set_cksum();
    archive.append_data(&mut header, "bin/tool", &b"new"[..])?;
    archive.finish()?;
    let mut digest = CasDigestData::digester_for_algorithm(DigestAlgorithm::Sha256);
    digest.update(&fs::read(directory.path().join("rootfs.tar"))?);
    pins.get_mut("rootfs").unwrap().sha256 = digest.finalize().raw_digest().to_string();
    fs::write(&path, serde_json::to_vec(&pins)?)?;
    let replacement = Runtime::load(&path, LAUNCHER_DIGEST, Some(&original))?;
    assert_ne!(original.digest(), replacement.digest());
    assert_eq!(fs::read(original.root().join("bin/tool"))?, b"tool");
    assert_eq!(fs::read(replacement.root().join("bin/tool"))?, b"new");
    let original_root = original.root();
    drop(original);
    assert!(!original_root.exists());
    assert!(replacement.root().exists());
    Ok(())
}

#[test]
fn a_manifest_cannot_authorize_its_own_launcher() -> bsmr_error::Result<()> {
    let directory = tempfile::tempdir()?;
    let path = manifest(directory.path(), false)?;
    fs::write(directory.path().join("bwrap"), b"untrusted")?;
    let mut manifest: BTreeMap<String, BundleArtifact> = serde_json::from_slice(&fs::read(&path)?)?;
    let mut digest = CasDigestData::digester_for_algorithm(DigestAlgorithm::Sha256);
    digest.update(b"untrusted");
    manifest.get_mut("bubblewrap").unwrap().sha256 = digest.finalize().raw_digest().to_string();
    fs::write(&path, serde_json::to_vec(&manifest)?)?;
    assert!(
        Runtime::load(&path, LAUNCHER_DIGEST, None)
            .unwrap_err()
            .to_string()
            .contains("trusted launcher")
    );
    Ok(())
}

#[test]
fn oversized_runtime_entries_fail_before_payload_materialization() -> bsmr_error::Result<()> {
    let mut archive = tempfile::tempfile()?;
    let mut header = tar::Header::new_gnu();
    header.set_path("oversized")?;
    header.set_size(MAX_RUNTIME_BYTES + 1);
    header.set_cksum();
    archive.write_all(header.as_bytes())?;
    archive.seek(SeekFrom::Start(0))?;
    let directory = tempfile::tempdir()?;
    assert!(
        unpack_runtime(archive, directory.path())
            .unwrap_err()
            .to_string()
            .contains("exceeds")
    );
    assert!(!directory.path().join("oversized").exists());
    Ok(())
}

/// File aliases may reference an earlier payload, never a directory or another alias.
#[test]
fn invariant_runtime_links_share_only_verified_payloads() -> bsmr_error::Result<()> {
    for (target, size, valid) in [
        ("bin/tool", 0, true),
        ("bin/tool", 1, false),
        ("../outside", 0, false),
        ("/outside", 0, false),
        ("missing", 0, false),
        ("", 0, false),
        ("bin", 0, false),
        ("bin/first", 0, false),
        ("bin/alias", 0, false),
    ] {
        let mut archive = tar::Builder::new(tempfile::tempfile()?);
        let mut header = tar::Header::new_gnu();
        header.set_mode(0o755);
        header.set_size(4);
        header.set_cksum();
        archive.append_data(&mut header, "bin/tool", &b"tool"[..])?;
        for (name, link, size) in [("bin/first", "bin/tool", 0), ("bin/alias", target, size)] {
            let mut header = tar::Header::new_gnu();
            header.set_entry_type(tar::EntryType::Link);
            header.set_mode(0o600);
            header.set_size(size);
            if !link.is_empty() {
                header.set_link_name(link)?;
            }
            header.set_cksum();
            archive.append_data(&mut header, name, std::io::repeat(b'x').take(size))?;
        }
        let mut archive = archive.into_inner()?;
        archive.seek(SeekFrom::Start(0))?;
        let root = tempfile::tempdir()?;
        let result = unpack_runtime(archive, root.path());
        assert_eq!(result.is_ok(), valid, "{target}: {result:?}");
        assert_eq!(fs::read(root.path().join("bin/tool"))?, b"tool");
        if result.is_ok() {
            assert_eq!(fs::read(root.path().join("bin/alias"))?, b"tool");
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                let original = fs::metadata(root.path().join("bin/tool"))?;
                let alias = fs::metadata(root.path().join("bin/alias"))?;
                assert_eq!(original.ino(), alias.ino());
                assert_eq!(original.mode() & 0o777, 0o755);
            }
        }
    }
    Ok(())
}
