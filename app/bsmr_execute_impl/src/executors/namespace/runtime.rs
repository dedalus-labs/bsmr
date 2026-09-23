//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Copies a digest-pinned launcher and runtime archive into a private execution snapshot.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::fs::File;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use bsmr_common::cas_digest::CasDigestData;
use bsmr_common::cas_digest::DigestAlgorithm;
use bsmr_sandbox::BundleArtifact;

const MAX_RUNTIME_BYTES: u64 = 256 * 1024 * 1024;
const MAX_RUNTIME_ENTRIES: usize = 50_000;

/// A private runtime snapshot whose identity excludes its host filesystem location.
#[derive(Debug)]
pub(crate) struct Runtime {
    /// Owns the private launcher and extracted root filesystem.
    directory: tempfile::TempDir,
    /// Identifies the verified bytes independently of their host paths.
    digest: String,
}

#[derive(Debug, bsmr_error::Error)]
#[bsmr(tag = Input)]
enum RuntimeError {
    #[error("namespace runtime must pin exactly `bubblewrap` and `rootfs`")]
    Manifest,
    #[error("namespace runtime artifact must use one relative file name: {0:?}")]
    ArtifactPath(PathBuf),
    #[error("namespace runtime artifact is not a regular file: {0:?}")]
    ArtifactType(PathBuf),
    #[error("namespace runtime launcher {actual} does not match trusted launcher {expected}")]
    UntrustedLauncher { expected: String, actual: String },
    #[error("namespace runtime artifact {path:?} has digest {actual}, expected {expected}")]
    Digest {
        path: PathBuf,
        expected: String,
        actual: String,
    },
    #[error(
        "namespace runtime exceeds its {MAX_RUNTIME_BYTES}-byte or {MAX_RUNTIME_ENTRIES}-entry limit"
    )]
    Limit,
    #[error("namespace runtime entry must be a unique regular file or directory: {0:?}")]
    Entry(PathBuf),
}

impl Runtime {
    /// Verify both pinned files and retain a private runtime snapshot.
    ///
    /// The caller must supply a trusted launcher digest independently of the manifest.
    pub(crate) fn load(manifest_path: &Path, launcher_digest: &str) -> bsmr_error::Result<Self> {
        let manifest: BTreeMap<String, BundleArtifact> =
            serde_json::from_slice(&fs::read(manifest_path)?)?;
        if manifest.keys().map(String::as_str).collect::<Vec<_>>() != ["bubblewrap", "rootfs"] {
            return Err(RuntimeError::Manifest.into());
        }
        if manifest["bubblewrap"].sha256 != launcher_digest {
            return Err(RuntimeError::UntrustedLauncher {
                expected: launcher_digest.to_owned(),
                actual: manifest["bubblewrap"].sha256.clone(),
            }
            .into());
        }
        let parent = manifest_path
            .parent()
            .expect("runtime manifest file has a parent");
        let directory = tempfile::Builder::new().prefix("bsmr-runtime-").tempdir()?;
        let launcher = directory.path().join("bubblewrap");
        copy_verified(
            parent,
            &manifest["bubblewrap"],
            &mut File::create(&launcher)?,
        )?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&launcher, fs::Permissions::from_mode(0o555))?;
        }
        let mut archive = tempfile::tempfile()?;
        copy_verified(parent, &manifest["rootfs"], &mut archive)?;
        archive.seek(SeekFrom::Start(0))?;
        let root = directory.path().join("rootfs");
        fs::create_dir(&root)?;
        unpack_runtime(archive, &root)?;
        let mut digest = CasDigestData::digester_for_algorithm(DigestAlgorithm::Sha256);
        digest.update(manifest["bubblewrap"].sha256.as_bytes());
        digest.update(manifest["rootfs"].sha256.as_bytes());
        Ok(Self {
            directory,
            digest: digest.finalize().raw_digest().to_string(),
        })
    }

    /// Return the verified launcher retained for this runtime's lifetime.
    pub(crate) fn launcher(&self) -> PathBuf {
        self.directory.path().join("bubblewrap")
    }

    /// Return the private root filesystem mounted read-only by the executor.
    pub(crate) fn root(&self) -> PathBuf {
        self.directory.path().join("rootfs")
    }

    /// Return the content identity bound into every action using this runtime.
    pub(crate) fn digest(&self) -> &str {
        &self.digest
    }
}

/// Hash the exact bytes copied to private storage, rejecting changed or oversized artifacts.
fn copy_verified(
    parent: &Path,
    artifact: &BundleArtifact,
    destination: &mut File,
) -> bsmr_error::Result<()> {
    let mut components = artifact.path.components();
    if !matches!(components.next(), Some(Component::Normal(_))) || components.next().is_some() {
        return Err(RuntimeError::ArtifactPath(artifact.path.clone()).into());
    }
    let path = parent.join(&artifact.path);
    if !fs::symlink_metadata(&path)?.file_type().is_file() {
        return Err(RuntimeError::ArtifactType(path).into());
    }
    let mut source = File::open(&path)?;
    let mut digest = CasDigestData::digester_for_algorithm(DigestAlgorithm::Sha256);
    let mut bytes = 0_u64;
    let mut buffer = vec![0; 64 * 1024];
    loop {
        let read = source.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        bytes += read as u64;
        if bytes > MAX_RUNTIME_BYTES {
            return Err(RuntimeError::Limit.into());
        }
        digest.update(&buffer[..read]);
        destination.write_all(&buffer[..read])?;
    }
    let actual = digest.finalize().raw_digest().to_string();
    if actual != artifact.sha256 {
        return Err(RuntimeError::Digest {
            path,
            expected: artifact.sha256.clone(),
            actual,
        }
        .into());
    }
    Ok(())
}

/// Accept a bounded, symlink-free runtime so extraction cannot redirect a host filesystem write.
fn unpack_runtime(archive: File, root: &Path) -> bsmr_error::Result<()> {
    let mut archive = tar::Archive::new(archive);
    let mut paths = BTreeSet::new();
    let mut bytes = 0_u64;
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?.into_owned();
        let kind = entry.header().entry_type();
        if path.as_os_str().is_empty()
            || !path
                .components()
                .all(|component| matches!(component, Component::Normal(_)))
            || (!kind.is_file() && !kind.is_dir())
            || !paths.insert(path.clone())
        {
            return Err(RuntimeError::Entry(path).into());
        }
        bytes = bytes.checked_add(entry.size()).ok_or(RuntimeError::Limit)?;
        if bytes > MAX_RUNTIME_BYTES || paths.len() > MAX_RUNTIME_ENTRIES {
            return Err(RuntimeError::Limit.into());
        }
        let destination = root.join(path);
        fs::create_dir_all(destination.parent().expect("runtime entry has a parent"))?;
        entry.unpack(destination)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "runtime/tests.rs"]
mod tests;
