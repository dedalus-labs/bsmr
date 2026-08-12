//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Selects and locks exact official Go SDK releases for the native frontend.

//! Defines the exact SDK identity used by the native Go frontend.
//!
//! Network access exists only at the explicit selection boundary. The committed lock
//! then gives graph import and execution one offline-verifiable SDK identity across
//! every supported execution host.

mod acquisition;
mod manifest;

use std::fs;
use std::path::Path;
use std::path::PathBuf;

pub(crate) use acquisition::acquired_go;
pub(crate) use acquisition::install_sdk;
pub(crate) use acquisition::prepare_acquisition;
pub(crate) use manifest::write_configuration;
use serde::Deserialize;
use serde::Serialize;

pub(super) const GENERATED_BY: &str = "bsmr go toolchain";
const GO_RELEASES_URL: &str = "https://go.dev/dl/?mode=json&include=all";
pub(super) const LOCK_FILE: &str = ".bsmr-go-toolchain.json";
const SCHEMA: u32 = 1;
const SUPPORTED_HOSTS: [(&str, &str); 4] = [
    ("darwin", "amd64"),
    ("darwin", "arm64"),
    ("linux", "amd64"),
    ("linux", "arm64"),
];

/// One immutable official SDK archive.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct GoSdkArchive {
    /// Selects the archive for an execution host, not a compilation target.
    pub(super) os: String,
    /// Selects the archive for an execution host, not a compilation target.
    pub(super) arch: String,
    /// Constrains generated download URLs to Go's canonical archive naming scheme.
    pub(super) filename: String,
    /// Authenticates archive bytes and contributes to content identity.
    pub(super) sha256: String,
    /// Detects truncated payloads before archive extraction.
    pub(super) size: u64,
}

impl GoSdkArchive {
    /// Returns the archive digest recorded by go.dev.
    #[cfg(test)]
    pub(crate) fn sha256(&self) -> &str {
        &self.sha256
    }
}

/// Committed identity of the Go SDK used for graph import and execution.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct GoToolchainLock {
    /// Proves file ownership before Bessemer replaces generated configuration.
    generated_by: String,
    /// Prevents a newer lock shape from being interpreted with older semantics.
    schema: u32,
    /// Fixes the Go language and tool behavior used for graph import and execution.
    pub(super) version: String,
    /// Lets one committed lock select verified bytes on every supported host.
    pub(super) archives: Vec<GoSdkArchive>,
}

impl GoToolchainLock {
    /// Returns the normalized SDK version without Go's `go` prefix.
    pub(crate) fn version(&self) -> &str {
        &self.version
    }

    /// Returns host archives in deterministic platform order.
    #[cfg(test)]
    pub(crate) fn archives(&self) -> &[GoSdkArchive] {
        &self.archives
    }
}

/// One release record from Go's official download metadata endpoint.
#[derive(Debug, Deserialize)]
struct Release {
    /// Supplies the exact release identifier, including Go's `go` prefix.
    version: String,
    /// Excludes previews because Bessemer defaults to supported stable semantics.
    stable: bool,
    /// Provides platform archives from which the supported host matrix is selected.
    files: Vec<ReleaseFile>,
}

/// One downloadable file from an official Go release record.
#[derive(Debug, Deserialize)]
struct ReleaseFile {
    /// Becomes the canonical go.dev URL suffix and an injection-safe name check.
    filename: String,
    /// Identifies the execution operating system for this archive.
    os: String,
    /// Identifies the execution architecture for this archive.
    arch: String,
    /// Authenticates the downloaded archive.
    sha256: String,
    /// Allows the downloader to reject incomplete content.
    size: u64,
    /// Separates binary SDK archives from source, installer, and checksum records.
    kind: String,
}

/// Updates or verifies the native toolchain configuration.
pub(crate) async fn configure(
    root: &Path,
    requested: Option<&str>,
    update: bool,
    check: bool,
) -> Result<GoToolchainLock, GoToolchainError> {
    let lock = if check {
        let lock = read_lock(root)?;
        if let Some(requested) = requested {
            let requested = normalize_version(requested)?;
            if lock.version != requested {
                return Err(GoToolchainError::VersionMismatch {
                    locked: lock.version,
                    requested,
                });
            }
        }
        lock
    } else if requested.is_none() && !update && root.join(LOCK_FILE).is_file() {
        read_lock(root)?
    } else {
        resolve_release(requested).await?
    };
    write_configuration(root, &lock, check)?;
    Ok(lock)
}

/// Fetches official metadata only when selection cannot use the committed lock.
async fn resolve_release(requested: Option<&str>) -> Result<GoToolchainLock, GoToolchainError> {
    let client = bsmr_http::HttpClientBuilder::oss()
        .await
        .map_err(|error| GoToolchainError::Network(error.to_string()))?
        .build();
    let response = client
        .get(GO_RELEASES_URL)
        .await
        .map_err(|error| GoToolchainError::Network(error.to_string()))?;
    let bytes = bsmr_http::to_bytes(response.into_body())
        .await
        .map_err(|error| GoToolchainError::Network(error.to_string()))?;
    select_release(&bytes, requested)
}

/// Selects one complete stable release from the official Go release response.
pub(crate) fn select_release(
    bytes: &[u8],
    requested: Option<&str>,
) -> Result<GoToolchainLock, GoToolchainError> {
    let releases = serde_json::from_slice::<Vec<Release>>(bytes)
        .map_err(|error| GoToolchainError::ReleaseMetadata(error.to_string()))?;
    let requested = requested.map(normalize_version).transpose()?;
    let mut candidates = releases
        .into_iter()
        .filter(|release| release.stable)
        .map(|release| {
            let version = normalize_version(&release.version)?;
            let key = version_key(&version)?;
            Ok((key, version, release))
        })
        .collect::<Result<Vec<_>, GoToolchainError>>()?;
    candidates.sort_by_key(|(key, _, _)| *key);
    let (_, version, release) = candidates
        .into_iter()
        .rev()
        .find(|(_, version, _)| {
            requested
                .as_ref()
                .is_none_or(|requested| requested == version)
        })
        .ok_or_else(|| GoToolchainError::ReleaseNotFound(requested.clone()))?;
    let archives = select_archives(&release, &version)?;
    Ok(GoToolchainLock {
        generated_by: GENERATED_BY.to_owned(),
        schema: SCHEMA,
        version,
        archives,
    })
}

/// Requires the official archive for every execution host Bessemer supports.
fn select_archives(
    release: &Release,
    version: &str,
) -> Result<Vec<GoSdkArchive>, GoToolchainError> {
    SUPPORTED_HOSTS
        .iter()
        .map(|&(os, arch)| {
            let file = release
                .files
                .iter()
                .find(|file| file.kind == "archive" && file.os == os && file.arch == arch)
                .ok_or_else(|| GoToolchainError::MissingArchive {
                    version: version.to_owned(),
                    os,
                    arch,
                })?;
            let archive = GoSdkArchive {
                os: os.to_owned(),
                arch: arch.to_owned(),
                filename: file.filename.clone(),
                sha256: file.sha256.clone(),
                size: file.size,
            };
            validate_archive(&archive, version)?;
            Ok(archive)
        })
        .collect()
}

/// Reads and validates the committed toolchain identity without network access.
pub(crate) fn read_lock(root: &Path) -> Result<GoToolchainLock, GoToolchainError> {
    let path = root.join(LOCK_FILE);
    let bytes = fs::read(&path).map_err(|error| GoToolchainError::Read {
        path: path.clone(),
        message: error.to_string(),
    })?;
    let lock = serde_json::from_slice::<GoToolchainLock>(&bytes)
        .map_err(|error| GoToolchainError::Lock(error.to_string()))?;
    validate_lock(&lock)?;
    Ok(lock)
}

/// Normalizes and validates a release version.
fn normalize_version(version: &str) -> Result<String, GoToolchainError> {
    let version = version.strip_prefix("go").unwrap_or(version);
    let component_count = version.split('.').count();
    let valid = (1..=3).contains(&component_count)
        && version.split('.').all(|component| {
            !component.is_empty() && component.chars().all(|c| c.is_ascii_digit())
        });
    if !valid {
        return Err(GoToolchainError::InvalidVersion(version.to_owned()));
    }
    Ok(version.to_owned())
}

/// Converts one validated release into a numeric ordering key.
fn version_key(version: &str) -> Result<(u64, u64, u64), GoToolchainError> {
    let mut components = version.split('.').map(str::parse::<u64>);
    let major = components
        .next()
        .transpose()
        .map_err(|_| GoToolchainError::InvalidVersion(version.to_owned()))?
        .ok_or_else(|| GoToolchainError::InvalidVersion(version.to_owned()))?;
    let minor = components
        .next()
        .transpose()
        .map_err(|_| GoToolchainError::InvalidVersion(version.to_owned()))?
        .unwrap_or(0);
    let patch = components
        .next()
        .transpose()
        .map_err(|_| GoToolchainError::InvalidVersion(version.to_owned()))?
        .unwrap_or(0);
    Ok((major, minor, patch))
}

/// Validates a deserialized lock before it can affect generated code.
pub(super) fn validate_lock(lock: &GoToolchainLock) -> Result<(), GoToolchainError> {
    if lock.generated_by != GENERATED_BY || lock.schema != SCHEMA {
        return Err(GoToolchainError::Lock(
            "unsupported ownership marker or schema".to_owned(),
        ));
    }
    normalize_version(&lock.version)?;
    for (index, (os, arch)) in SUPPORTED_HOSTS.iter().enumerate() {
        let Some(archive) = lock.archives.get(index) else {
            return Err(GoToolchainError::MissingArchive {
                version: lock.version.clone(),
                os,
                arch,
            });
        };
        if archive.os != *os || archive.arch != *arch {
            return Err(GoToolchainError::Lock(
                "archives are not in canonical supported-host order".to_owned(),
            ));
        }
        validate_archive(archive, &lock.version)?;
    }
    if lock.archives.len() != SUPPORTED_HOSTS.len() {
        return Err(GoToolchainError::Lock(
            "lock contains an unsupported host archive".to_owned(),
        ));
    }
    Ok(())
}

/// Validates every release-controlled value interpolated into generated Starlark.
fn validate_archive(archive: &GoSdkArchive, version: &str) -> Result<(), GoToolchainError> {
    let expected = format!("go{version}.{}-{}.tar.gz", archive.os, archive.arch);
    if archive.filename != expected {
        return Err(GoToolchainError::UnexpectedArchiveName {
            expected,
            actual: archive.filename.clone(),
        });
    }
    if archive.sha256.is_empty() {
        return Err(GoToolchainError::MissingDigest(archive.filename.clone()));
    }
    if archive.sha256.len() != 64 || !archive.sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(GoToolchainError::InvalidDigest(archive.filename.clone()));
    }
    if archive.size == 0 {
        return Err(GoToolchainError::MissingSize(archive.filename.clone()));
    }
    Ok(())
}

/// Fail-closed SDK selection and generated-toolchain errors.
#[derive(Debug, bsmr_error::Error)]
#[bsmr(tag = Input)]
pub(crate) enum GoToolchainError {
    #[error(
        "invalid exact Go version `{0}`; expected `1`, `1.x`, `1.x.y`, or an optional `go` prefix"
    )]
    InvalidVersion(String),
    #[error("official Go release metadata is invalid: {0}")]
    ReleaseMetadata(String),
    #[error("official stable Go release {0:?} was not found")]
    ReleaseNotFound(Option<String>),
    #[error("requested Go {requested} does not match locked Go {locked}")]
    VersionMismatch { locked: String, requested: String },
    #[error("failed to fetch official Go release metadata: {0}")]
    Network(String),
    #[error("Go {version} has no official {os}-{arch} archive")]
    MissingArchive {
        version: String,
        os: &'static str,
        arch: &'static str,
    },
    #[error("official Go archive `{0}` has no SHA-256 digest")]
    MissingDigest(String),
    #[error("official Go archive `{0}` has an invalid SHA-256 digest")]
    InvalidDigest(String),
    #[error("official Go archive name `{actual}` does not match expected `{expected}`")]
    UnexpectedArchiveName { expected: String, actual: String },
    #[error("official Go archive `{0}` has no byte length")]
    MissingSize(String),
    #[error("Go SDK acquisition does not support host {0}-{1}")]
    UnsupportedHost(String, String),
    #[error("locked Go SDK is not acquired; run `bsmr go toolchain`")]
    NotAcquired,
    #[error("locked Go {locked} does not match acquired SDK `{declared}`")]
    SdkVersion { locked: String, declared: String },
    #[error("failed to copy acquired Go SDK: {0}")]
    Copy(String),
    #[error("failed to compile the Go bootstrap wrapper: {0}")]
    Wrapper(String),
    #[error("Go toolchain lock is invalid: {0}")]
    Lock(String),
    #[error("failed to render generated Go toolchain: {0}")]
    Render(String),
    #[error("failed to read `{path:?}`: {message}")]
    Read { path: PathBuf, message: String },
    #[error("failed to write `{path:?}`: {message}")]
    Write { path: PathBuf, message: String },
    #[error("refusing to overwrite user-owned toolchain file `{0:?}`")]
    UserOwned(PathBuf),
    #[error("generated Go toolchain files are stale: {0:?}")]
    Stale(Vec<PathBuf>),
}
