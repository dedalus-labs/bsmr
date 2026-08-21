//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Creates digest-pinned manifests for operator-built Firecracker bundles.

use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;

use bsmr_sandbox::BundleArtifact;
use bsmr_sandbox::BundleManifest;
use bsmr_sandbox::PROTOCOL_VERSION;
use clap::Parser;
use sha2::Digest;
use sha2::Sha256;
use thiserror::Error;

#[derive(Debug, Error)]
enum BundleBuildError {
    #[error("failed to identify Firecracker host: {0}")]
    Bundle(#[from] bsmr_sandbox::BundleError),
    #[cfg(target_os = "linux")]
    #[error("failed to create pristine Firecracker snapshot: {0}")]
    Snapshot(#[from] bsmr_sandbox::snapshot::SnapshotError),
    #[cfg(not(target_os = "linux"))]
    #[error("pristine Firecracker snapshots require Linux KVM")]
    UnsupportedHost,
    #[error("version, architecture, and host fingerprint must be non-empty")]
    EmptyIdentity,
    #[error("failed to read bundle artifact {path}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to write bundle manifest {path}: {source}")]
    Write {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to serialize bundle manifest: {0}")]
    Serialize(#[from] serde_json::Error),
}

#[derive(Debug, Parser)]
#[command(name = "bsmr-sandbox-bundle")]
struct Args {
    /// Directory containing firecracker, jailer, kernel, and rootfs.
    #[arg(long)]
    directory: PathBuf,
    /// Exact Firecracker and jailer release without the leading `v`.
    #[arg(long)]
    firecracker_version: String,
    /// Bundle architecture.
    #[arg(long, default_value = std::env::consts::ARCH)]
    architecture: String,
}

/// Writes one new manifest or exits without changing an existing bundle.
fn main() -> Result<(), BundleBuildError> {
    let args = Args::parse();
    let host_fingerprint = current_host_fingerprint()?;
    create_snapshot(&args.directory)?;
    write_manifest(
        &args.directory,
        &args.firecracker_version,
        &args.architecture,
        &host_fingerprint,
    )?;
    Ok(())
}

#[cfg(target_os = "linux")]
/// Identifies the host contract serialized into the snapshot.
fn current_host_fingerprint() -> Result<String, BundleBuildError> {
    Ok(bsmr_sandbox::host_fingerprint()?)
}

#[cfg(not(target_os = "linux"))]
/// Fails rather than invent a host identity without Linux KVM.
fn current_host_fingerprint() -> Result<String, BundleBuildError> {
    Err(BundleBuildError::UnsupportedHost)
}

#[cfg(target_os = "linux")]
/// Creates the pristine state bundled by the manifest on supported hosts.
fn create_snapshot(directory: &Path) -> Result<(), BundleBuildError> {
    bsmr_sandbox::snapshot::create(directory)?;
    Ok(())
}

#[cfg(not(target_os = "linux"))]
/// Fails rather than emit a manifest without a real pristine snapshot.
fn create_snapshot(_: &Path) -> Result<(), BundleBuildError> {
    Err(BundleBuildError::UnsupportedHost)
}

/// Hashes every required artifact and creates a stable manifest atomically.
fn write_manifest(
    directory: &Path,
    version: &str,
    architecture: &str,
    host_fingerprint: &str,
) -> Result<(), BundleBuildError> {
    if version.is_empty() || architecture.is_empty() || host_fingerprint.is_empty() {
        return Err(BundleBuildError::EmptyIdentity);
    }
    let artifacts = [
        "firecracker",
        "jailer",
        "kernel",
        "rootfs",
        "snapshot",
        "memory",
    ]
    .into_iter()
    .map(|name| {
        Ok((
            name.to_owned(),
            BundleArtifact {
                path: name.into(),
                sha256: sha256_file(&directory.join(name))?,
            },
        ))
    })
    .collect::<Result<BTreeMap<_, _>, BundleBuildError>>()?;
    let manifest = BundleManifest {
        schema: PROTOCOL_VERSION,
        architecture: architecture.to_owned(),
        host_fingerprint: host_fingerprint.to_owned(),
        firecracker_version: version.to_owned(),
        jailer_version: version.to_owned(),
        artifacts,
    };
    let path = directory.join("manifest.json");
    let mut file = File::options()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|source| BundleBuildError::Write {
            path: path.clone(),
            source,
        })?;
    serde_json::to_writer_pretty(&mut file, &manifest)?;
    file.write_all(b"\n")
        .and_then(|()| file.sync_all())
        .map_err(|source| BundleBuildError::Write { path, source })?;
    Ok(())
}

/// Calculates the lowercase SHA-256 identity of one artifact.
fn sha256_file(path: &Path) -> Result<String, BundleBuildError> {
    let mut file = File::open(path).map_err(|source| BundleBuildError::Read {
        path: path.to_owned(),
        source,
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 16 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|source| BundleBuildError::Read {
                path: path.to_owned(),
                source,
            })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use std::fs;

    use bsmr_sandbox::BundleTrust;
    use bsmr_sandbox::VerifiedBundle;

    use super::write_manifest;

    /// The manifest binds the complete boot and pristine-resume state.
    #[test]
    fn manifest_is_complete_and_create_only() {
        let directory = tempfile::tempdir().unwrap();
        for name in [
            "firecracker",
            "jailer",
            "kernel",
            "rootfs",
            "snapshot",
            "memory",
        ] {
            fs::write(directory.path().join(name), name).unwrap();
        }

        write_manifest(
            directory.path(),
            "1.16.1",
            std::env::consts::ARCH,
            "sha256:test-host",
        )
        .unwrap();

        let bundle = VerifiedBundle::load(
            &directory.path().join("manifest.json"),
            std::env::consts::ARCH,
            BundleTrust::Content,
        )
        .unwrap();
        assert!(bundle.environment_digest().starts_with("sha256:"));
        assert!(bundle.artifact("snapshot").unwrap().is_file());
        assert!(bundle.artifact("memory").unwrap().is_file());
        assert!(
            write_manifest(
                directory.path(),
                "1.16.1",
                std::env::consts::ARCH,
                "sha256:test-host"
            )
            .is_err()
        );
    }
}
