//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Defines the versioned protocol shared by BSMR's Firecracker components.

#[cfg(target_os = "linux")]
pub mod firecracker;
#[cfg(target_os = "linux")]
pub mod snapshot;

use std::collections::BTreeMap;
use std::fs;
use std::fs::File;
use std::io::Read;
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;
use serde::Serialize;
use sha2::Digest;
use sha2::Sha256;
use thiserror::Error;

pub const PROTOCOL_VERSION: u32 = 2;
pub const MAX_ACTION_BYTES: u64 = 64 * 1024;
pub const MAX_INPUT_BYTES: u64 = 1024 * 1024 * 1024;
pub const MAX_OUTPUT_ARCHIVE_BYTES: u64 = 1024 * 1024 * 1024;
pub const MAX_OUTPUT_BYTES: u64 = MAX_OUTPUT_ARCHIVE_BYTES + 64 * 1024 * 1024;
pub const MAX_STREAM_BYTES: u64 = 16 * 1024 * 1024;
pub const MAX_TIMEOUT_MS: u64 = 24 * 60 * 60 * 1000;
pub const VCPU_COUNT: u8 = 2;
pub const MEMORY_MIB: u32 = 2048;

#[derive(Debug, Error)]
pub enum BundleError {
    #[error("failed to read bundle path {path:?}: {source}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("invalid bundle manifest: {0}")]
    Manifest(#[from] serde_json::Error),
    #[error("bundle schema must be {PROTOCOL_VERSION}, got {0}")]
    Schema(u32),
    #[error("bundle architecture {bundle:?} does not match host {host:?}")]
    Architecture { bundle: String, host: String },
    #[error("bundle host fingerprint {bundle:?} does not match host {host:?}")]
    HostFingerprint { bundle: String, host: String },
    #[error("Firecracker and jailer versions differ: {firecracker:?} and {jailer:?}")]
    Version { firecracker: String, jailer: String },
    #[error("bundle is missing required artifact {0:?}")]
    Missing(&'static str),
    #[error("bundle artifact path must be one relative file name: {0:?}")]
    Path(PathBuf),
    #[error("bundle artifact is not a regular file: {0:?}")]
    Type(PathBuf),
    #[error("bundle artifact {path:?} has digest {actual}, expected {expected}")]
    Digest {
        path: PathBuf,
        expected: String,
        actual: String,
    },
    #[error("bundle path is not immutable and root-owned: {0:?}")]
    Ownership(PathBuf),
    #[error("bundle executable is not a supported static ELF file: {0:?}")]
    Elf(PathBuf),
}

/// Ownership policy applied while loading an execution bundle.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BundleTrust {
    Content,
    RootOwned,
}

/// A verified immutable execution bundle shared by the host and launcher.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VerifiedBundle {
    root: PathBuf,
    manifest: BundleManifest,
    environment_digest: String,
}

impl VerifiedBundle {
    /// Verifies a manifest and every bound artifact against the selected host.
    pub fn load(
        manifest_path: &Path,
        host_architecture: &str,
        trust: BundleTrust,
    ) -> Result<Self, BundleError> {
        let manifest_path =
            fs::canonicalize(manifest_path).map_err(|source| BundleError::Read {
                path: manifest_path.to_owned(),
                source,
            })?;
        if trust == BundleTrust::RootOwned {
            verify_root_owned_chain(&manifest_path)?;
        }
        let manifest_metadata =
            fs::symlink_metadata(&manifest_path).map_err(|source| BundleError::Read {
                path: manifest_path.clone(),
                source,
            })?;
        if !manifest_metadata.file_type().is_file() {
            return Err(BundleError::Type(manifest_path));
        }
        let file = File::open(&manifest_path).map_err(|source| BundleError::Read {
            path: manifest_path.clone(),
            source,
        })?;
        let manifest: BundleManifest = serde_json::from_reader(file)?;
        if manifest.schema != PROTOCOL_VERSION {
            return Err(BundleError::Schema(manifest.schema));
        }
        if manifest.architecture != host_architecture {
            return Err(BundleError::Architecture {
                bundle: manifest.architecture,
                host: host_architecture.to_owned(),
            });
        }
        #[cfg(target_os = "linux")]
        if trust == BundleTrust::RootOwned {
            let host = host_fingerprint()?;
            verify_host_identity(&manifest.host_fingerprint, &host)?;
        }
        if manifest.firecracker_version != manifest.jailer_version {
            return Err(BundleError::Version {
                firecracker: manifest.firecracker_version,
                jailer: manifest.jailer_version,
            });
        }
        for required in [
            "firecracker",
            "jailer",
            "kernel",
            "rootfs",
            "snapshot",
            "memory",
        ] {
            if !manifest.artifacts.contains_key(required) {
                return Err(BundleError::Missing(required));
            }
        }
        let root = manifest_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .to_owned();
        for artifact in manifest.artifacts.values() {
            let mut components = artifact.path.components();
            if !matches!(components.next(), Some(Component::Normal(_)))
                || components.next().is_some()
            {
                return Err(BundleError::Path(artifact.path.clone()));
            }
            let path = root.join(&artifact.path);
            if trust == BundleTrust::RootOwned {
                verify_root_owned(&path)?;
            }
            let metadata = fs::symlink_metadata(&path).map_err(|source| BundleError::Read {
                path: path.clone(),
                source,
            })?;
            if !metadata.file_type().is_file() {
                return Err(BundleError::Type(path));
            }
            verify_sha256(&path, &artifact.sha256)?;
        }
        if trust == BundleTrust::RootOwned {
            verify_static_elf(
                &root.join(&manifest.artifacts["firecracker"].path),
                host_architecture,
            )?;
            verify_static_elf(
                &root.join(&manifest.artifacts["jailer"].path),
                host_architecture,
            )?;
        }
        let canonical = serde_json::to_vec(&manifest)?;
        let environment_digest = format!("sha256:{:x}", Sha256::digest(canonical));
        Ok(Self {
            root,
            manifest,
            environment_digest,
        })
    }

    /// Returns the semantic digest included in sandboxed action keys.
    #[must_use]
    pub fn environment_digest(&self) -> &str {
        &self.environment_digest
    }

    /// Resolves one required artifact below the verified bundle root.
    pub fn artifact(&self, name: &'static str) -> Result<PathBuf, BundleError> {
        let artifact = self
            .manifest
            .artifacts
            .get(name)
            .ok_or(BundleError::Missing(name))?;
        Ok(self.root.join(&artifact.path))
    }

    /// Returns one verified artifact's lowercase content identity.
    pub fn artifact_sha256(&self, name: &'static str) -> Result<&str, BundleError> {
        self.manifest
            .artifacts
            .get(name)
            .map(|artifact| artifact.sha256.as_str())
            .ok_or(BundleError::Missing(name))
    }

    /// Returns the release version both Firecracker executables must report.
    #[must_use]
    pub fn firecracker_version(&self) -> &str {
        &self.manifest.firecracker_version
    }
}

/// Rejects a snapshot built against a different CPU, microcode, or host kernel.
#[cfg(target_os = "linux")]
fn verify_host_identity(bundle: &str, host: &str) -> Result<(), BundleError> {
    if bundle != host {
        return Err(BundleError::HostFingerprint {
            bundle: bundle.to_owned(),
            host: host.to_owned(),
        });
    }
    Ok(())
}

/// Rejects dynamic or wrong-architecture VMM executables.
fn verify_static_elf(path: &Path, architecture: &str) -> Result<(), BundleError> {
    let mut file = File::open(path).map_err(|source| BundleError::Read {
        path: path.to_owned(),
        source,
    })?;
    let mut header = [0u8; 64];
    file.read_exact(&mut header)
        .map_err(|_| BundleError::Elf(path.to_owned()))?;
    let machine = match architecture {
        "x86_64" => 62,
        "aarch64" => 183,
        _ => return Err(BundleError::Elf(path.to_owned())),
    };
    if &header[..4] != b"\x7fELF"
        || header[4] != 2
        || header[5] != 1
        || u16::from_le_bytes(header[18..20].try_into().expect("ELF machine field")) != machine
    {
        return Err(BundleError::Elf(path.to_owned()));
    }
    let program_offset = u64::from_le_bytes(
        header[32..40]
            .try_into()
            .expect("ELF program-header offset"),
    );
    let entry_bytes =
        u16::from_le_bytes(header[54..56].try_into().expect("ELF program-header size")) as u64;
    let entries =
        u16::from_le_bytes(header[56..58].try_into().expect("ELF program-header count")) as u64;
    if entry_bytes < 4 || entries == 0 || entries > 1024 {
        return Err(BundleError::Elf(path.to_owned()));
    }
    let mut kind = [0u8; 4];
    for index in 0..entries {
        use std::io::Seek;
        use std::io::SeekFrom;

        let offset = index
            .checked_mul(entry_bytes)
            .and_then(|offset| program_offset.checked_add(offset))
            .ok_or_else(|| BundleError::Elf(path.to_owned()))?;
        file.seek(SeekFrom::Start(offset))
            .and_then(|_| file.read_exact(&mut kind))
            .map_err(|_| BundleError::Elf(path.to_owned()))?;
        if u32::from_le_bytes(kind) == 3 {
            return Err(BundleError::Elf(path.to_owned()));
        }
    }
    Ok(())
}

/// Calculates one artifact's lowercase SHA-256 digest.
fn sha256_file(path: &Path) -> Result<String, BundleError> {
    let mut file = File::open(path).map_err(|source| BundleError::Read {
        path: path.to_owned(),
        source,
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = [0; 16 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|source| BundleError::Read {
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

/// Verifies one regular file against an expected lowercase SHA-256 digest.
pub fn verify_sha256(path: &Path, expected: &str) -> Result<(), BundleError> {
    let actual = sha256_file(path)?;
    if actual != expected {
        return Err(BundleError::Digest {
            path: path.to_owned(),
            expected: expected.to_owned(),
            actual,
        });
    }
    Ok(())
}

#[cfg(unix)]
/// Rejects artifacts writable by identities other than root.
fn verify_root_owned(path: &Path) -> Result<(), BundleError> {
    use std::os::unix::fs::MetadataExt;

    let metadata = fs::symlink_metadata(path).map_err(|source| BundleError::Read {
        path: path.to_owned(),
        source,
    })?;
    if metadata.uid() != 0 || metadata.mode() & 0o022 != 0 {
        return Err(BundleError::Ownership(path.to_owned()));
    }
    Ok(())
}

#[cfg(unix)]
/// Applies the immutable ownership rule to a path and every ancestor.
fn verify_root_owned_chain(path: &Path) -> Result<(), BundleError> {
    for ancestor in path.ancestors() {
        verify_root_owned(ancestor)?;
    }
    Ok(())
}

#[cfg(not(unix))]
/// Fails ownership verification on hosts without Unix ownership semantics.
fn verify_root_owned(path: &Path) -> Result<(), BundleError> {
    Err(BundleError::Ownership(path.to_owned()))
}

#[cfg(not(unix))]
/// Fails ownership-chain verification on unsupported hosts.
fn verify_root_owned_chain(path: &Path) -> Result<(), BundleError> {
    Err(BundleError::Ownership(path.to_owned()))
}

/// One digest-pinned file in a Firecracker execution bundle.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BundleArtifact {
    pub path: PathBuf,
    pub sha256: String,
}

/// The semantic contents of one immutable Firecracker bundle.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct BundleManifest {
    pub schema: u32,
    pub architecture: String,
    pub host_fingerprint: String,
    pub firecracker_version: String,
    pub jailer_version: String,
    pub artifacts: BTreeMap<String, BundleArtifact>,
}

#[cfg(target_os = "linux")]
/// Identifies the CPU, microcode, and KVM host kernel used by a snapshot.
pub fn host_fingerprint() -> Result<String, BundleError> {
    let cpu_path = Path::new("/proc/cpuinfo");
    let cpu = fs::read_to_string(cpu_path).map_err(|source| BundleError::Read {
        path: cpu_path.to_owned(),
        source,
    })?;
    let kernel_path = Path::new("/proc/sys/kernel/osrelease");
    let kernel = fs::read_to_string(kernel_path).map_err(|source| BundleError::Read {
        path: kernel_path.to_owned(),
        source,
    })?;
    let mut identity = cpu
        .lines()
        .take_while(|line| !line.is_empty())
        .filter(|line| {
            let field = line.split_once(':').map(|(field, _)| field.trim());
            field.is_some_and(|field| {
                [
                    "vendor_id",
                    "cpu family",
                    "model",
                    "model name",
                    "stepping",
                    "microcode",
                    "flags",
                ]
                .contains(&field)
            })
        })
        .collect::<Vec<_>>()
        .join("\n");
    identity.push_str("\nkernel = ");
    identity.push_str(kernel.trim());
    Ok(format!("sha256:{:x}", Sha256::digest(identity)))
}

/// The shape of one output admitted by the guest result validator.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum GuestOutputKind {
    File,
    Directory,
    FileOrDirectory,
}

/// One normalized output declaration sent to the guest.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GuestOutput {
    pub path: PathBuf,
    pub kind: GuestOutputKind,
}

impl GuestOutput {
    /// Declares one exact output file.
    #[must_use]
    pub fn file(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            kind: GuestOutputKind::File,
        }
    }

    /// Declares one output directory and its descendants.
    #[must_use]
    pub fn directory(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            kind: GuestOutputKind::Directory,
        }
    }
}

/// The canonical action consumed by the guest PID 1.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GuestAction {
    pub protocol: u32,
    pub arguments: Vec<String>,
    pub environment: BTreeMap<String, String>,
    pub working_directory: PathBuf,
    pub outputs: Vec<GuestOutput>,
    pub timeout_ms: Option<u64>,
}

/// The fixed-size request sent to the privileged launcher with three file descriptors.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LauncherRequest {
    pub protocol: u32,
    pub action_id: String,
    pub environment_digest: String,
    pub input_bytes: u64,
    pub input_sha256: String,
    pub output_bytes: u64,
    pub action_sha256: String,
    pub vcpu_count: u8,
    pub memory_mib: u32,
    pub timeout_ms: Option<u64>,
}

/// The terminal state returned by the privileged launcher.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LauncherStatus {
    Completed,
    TimedOut,
    Cancelled,
    Failed,
}

/// The bounded response returned after the launcher has torn down the microVM.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LauncherResponse {
    pub protocol: u32,
    pub status: LauncherStatus,
    pub cleanup_complete: bool,
    pub environment_start_us: Option<u64>,
    pub error: Option<String>,
}

/// The action result written inside the untrusted guest output archive.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct GuestResultEnvelope {
    pub protocol: u32,
    pub exit_code: i32,
    #[serde(default)]
    pub timed_out: bool,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::io::Seek;
    use std::io::SeekFrom;
    use std::io::Write;

    use sha2::Digest;
    use sha2::Sha256;

    use super::BundleArtifact;
    use super::BundleError;
    use super::BundleManifest;
    use super::BundleTrust;
    use super::PROTOCOL_VERSION;
    use super::VerifiedBundle;
    #[cfg(target_os = "linux")]
    use super::verify_host_identity;
    use super::verify_static_elf;

    /// The bundle verifier accepts a matching ELF64 machine with no interpreter.
    #[test]
    fn static_elf_has_no_interpreter_segment() {
        let file = synthetic_elf(1);

        assert!(verify_static_elf(file.path(), "x86_64").is_ok());
        assert!(verify_static_elf(file.path(), "aarch64").is_err());
    }

    /// A PT_INTERP segment identifies a dynamically linked executable and is rejected.
    #[test]
    fn dynamic_elf_is_rejected() {
        let file = synthetic_elf(3);

        assert!(verify_static_elf(file.path(), "x86_64").is_err());
    }

    /// Snapshot state cannot cross an unproven CPU and host-kernel boundary.
    #[cfg(target_os = "linux")]
    #[test]
    fn snapshot_host_identity_is_exact() {
        assert!(verify_host_identity("sha256:host", "sha256:host").is_ok());
        assert!(verify_host_identity("sha256:source", "sha256:target").is_err());
    }

    /// A manifest cannot bless changed bytes or two different executable releases.
    #[test]
    fn bundle_rejects_digest_and_version_mismatches() {
        let directory = tempfile::tempdir().unwrap();
        let mut artifacts = BTreeMap::new();
        for name in [
            "firecracker",
            "jailer",
            "kernel",
            "rootfs",
            "snapshot",
            "memory",
        ] {
            fs::write(directory.path().join(name), name).unwrap();
            artifacts.insert(
                name.to_owned(),
                BundleArtifact {
                    path: name.into(),
                    sha256: format!("{:x}", Sha256::digest(name.as_bytes())),
                },
            );
        }
        let mut manifest = BundleManifest {
            schema: PROTOCOL_VERSION,
            architecture: std::env::consts::ARCH.to_owned(),
            host_fingerprint: "sha256:test-host".to_owned(),
            firecracker_version: "1.16.1".to_owned(),
            jailer_version: "1.16.1".to_owned(),
            artifacts,
        };
        let path = directory.path().join("manifest.json");
        fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        fs::write(directory.path().join("kernel"), "changed").unwrap();
        assert!(VerifiedBundle::load(&path, std::env::consts::ARCH, BundleTrust::Content).is_err());

        manifest.jailer_version = "1.16.0".to_owned();
        fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        assert!(VerifiedBundle::load(&path, std::env::consts::ARCH, BundleTrust::Content).is_err());
    }

    /// A production bundle is incomplete without pristine VM state and memory.
    #[test]
    fn bundle_requires_pristine_snapshot_artifacts() {
        let directory = tempfile::tempdir().unwrap();
        let mut artifacts = BTreeMap::new();
        for name in ["firecracker", "jailer", "kernel", "rootfs"] {
            fs::write(directory.path().join(name), name).unwrap();
            artifacts.insert(
                name.to_owned(),
                BundleArtifact {
                    path: name.into(),
                    sha256: format!("{:x}", Sha256::digest(name.as_bytes())),
                },
            );
        }
        let manifest = BundleManifest {
            schema: PROTOCOL_VERSION,
            architecture: std::env::consts::ARCH.to_owned(),
            host_fingerprint: "sha256:test-host".to_owned(),
            firecracker_version: "1.16.1".to_owned(),
            jailer_version: "1.16.1".to_owned(),
            artifacts,
        };
        let path = directory.path().join("manifest.json");
        fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();

        assert!(matches!(
            VerifiedBundle::load(&path, std::env::consts::ARCH, BundleTrust::Content),
            Err(BundleError::Missing("snapshot"))
        ));
    }

    /// Root ownership cannot compensate for a group- or world-writable artifact.
    #[cfg(unix)]
    #[test]
    fn bundle_ownership_rejects_writable_artifacts() {
        use std::os::unix::fs::PermissionsExt;

        let file = tempfile::NamedTempFile::new().unwrap();
        file.as_file()
            .set_permissions(fs::Permissions::from_mode(0o666))
            .unwrap();
        assert!(super::verify_root_owned(file.path()).is_err());
    }

    /// Writes one minimal ELF64 header and program header for verifier tests.
    fn synthetic_elf(program_kind: u32) -> tempfile::NamedTempFile {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        let mut header = [0u8; 64];
        header[..4].copy_from_slice(b"\x7fELF");
        header[4] = 2;
        header[5] = 1;
        header[18..20].copy_from_slice(&62u16.to_le_bytes());
        header[32..40].copy_from_slice(&64u64.to_le_bytes());
        header[54..56].copy_from_slice(&56u16.to_le_bytes());
        header[56..58].copy_from_slice(&1u16.to_le_bytes());
        file.write_all(&header).unwrap();
        file.write_all(&program_kind.to_le_bytes()).unwrap();
        file.as_file_mut().seek(SeekFrom::Start(0)).unwrap();
        file
    }
}
