//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Defines typed DependencySet digest kinds, errors, contract ranges, and validators.

//! Cross-repository DependencySet contracts, exact locks, and release receipts.
//!
//! A DependencySet encodes the authoritative rule graph of component manifests
//! (`bsmr.component.toml`) under `bsmr.dependency-set.v1`. A DependencyLock
//! encodes one exact resolution (`bsmr.lock`) under `bsmr.dependency-lock.v1`.
//! A ReleaseReceipt certifies one verified lock and its produced artifacts
//! under `bsmr.release-receipt.v1`.

mod lock;
mod manifest;
mod range;
mod verify;

use sha2::Digest;
use sha2::Sha256;

pub use self::lock::DependencyLock;
pub use self::lock::LockedArtifact;
pub use self::lock::LockedComponent;
pub use self::lock::LockedLockfile;
pub use self::lock::LockfileEcosystem;
pub use self::manifest::ComponentManifest;
pub use self::manifest::DependencySet;
pub use self::manifest::PatchPolicy;
pub use self::manifest::ProvenancePolicy;
pub use self::range::ContractRange;
pub use self::range::ContractRangeSet;
pub use self::verify::ContractRequirer;
pub use self::verify::ContractResolution;
pub use self::verify::VerifiedResolution;
pub use self::verify::load_project_state;
pub use self::verify::refresh_lockfiles;
pub use self::verify::verify;
pub use self::verify::verify_in_root;
pub use self::verify::verify_project_before_build;
use crate::cas_digest::CasDigest;
use crate::cas_digest::CasDigestConfig;
use crate::cas_digest::CasDigestKind;
use crate::cas_digest::RawDigest;
use crate::cas_digest::TrackedCasDigest;

/// Domain prefix for canonical `DependencySet` rule-graph objects in CAS.
pub const DEPENDENCY_SET_FORMAT: &[u8] = b"bsmr.dependency-set.v1\0";

/// Domain prefix for canonical `DependencyLock` resolved-graph objects in CAS.
pub const DEPENDENCY_LOCK_FORMAT: &[u8] = b"bsmr.dependency-lock.v1\0";

/// Domain prefix for canonical `ReleaseReceipt` objects in CAS.
pub const RELEASE_RECEIPT_FORMAT: &[u8] = b"bsmr.release-receipt.v1\0";

/// Default manifest filename at a component root.
pub const COMPONENT_MANIFEST_FILE: &str = "bsmr.component.toml";

/// Default lockfile filename at a repository root.
pub const DEPENDENCY_LOCK_FILE: &str = "bsmr.lock";

/// Prevents a DependencySet object from being confused with another CAS blob.
pub struct DependencySetDigestKind {
    _private: (),
}

impl CasDigestKind for DependencySetDigestKind {
    fn empty_digest(_config: CasDigestConfig) -> Option<TrackedCasDigest<Self>> {
        None
    }
}

/// Content identity of a canonical DependencySet CAS object.
pub type DependencySetDigest = CasDigest<DependencySetDigestKind>;

/// Prevents a DependencyLock object from being confused with another CAS blob.
pub struct DependencyLockDigestKind {
    _private: (),
}

impl CasDigestKind for DependencyLockDigestKind {
    fn empty_digest(_config: CasDigestConfig) -> Option<TrackedCasDigest<Self>> {
        None
    }
}

/// Content identity of a canonical DependencyLock CAS object.
pub type DependencyLockDigest = CasDigest<DependencyLockDigestKind>;

/// Prevents a ReleaseReceipt object from being confused with another CAS blob.
pub struct ReleaseReceiptDigestKind {
    _private: (),
}

impl CasDigestKind for ReleaseReceiptDigestKind {
    fn empty_digest(_config: CasDigestConfig) -> Option<TrackedCasDigest<Self>> {
        None
    }
}

/// Content identity of a canonical ReleaseReceipt CAS object.
pub type ReleaseReceiptDigest = CasDigest<ReleaseReceiptDigestKind>;

/// Typed errors emitted when parsing, verifying, updating, or certifying a DependencySet.
#[derive(Debug, bsmr_error::Error)]
#[bsmr(tag = Input)]
pub enum DependencySetError {
    /// Failed to read or write a DependencySet file.
    #[error("{operation} `{path}`: {message}")]
    Io {
        operation: &'static str,
        path: String,
        message: String,
    },

    /// A component manifest failed schema or field validation.
    #[error("invalid component manifest: {0}")]
    InvalidManifest(String),

    /// A dependency lock failed schema or field validation.
    #[error("invalid dependency lock: {0}")]
    InvalidLock(String),

    /// A candidate catalog failed schema, digest, or field validation.
    #[error("invalid candidate catalog: {0}")]
    InvalidCatalog(String),

    /// A release receipt failed schema, policy, or digest validation.
    #[error("invalid release receipt: {0}")]
    InvalidReceipt(String),

    /// A component name failed canonical identifier validation.
    #[error("invalid component name `{name}`: {reason}")]
    InvalidComponentName { name: String, reason: String },

    /// A contract identifier failed dot-separated naming validation.
    #[error("invalid contract name `{name}`: {reason}")]
    InvalidContractName { name: String, reason: String },

    /// A Git commit SHA was not 40 lowercase hexadecimal characters.
    #[error("invalid commit revision `{revision}`: expected 40 lowercase hexadecimal characters")]
    InvalidCommitSha { revision: String },

    /// A SHA-256 content digest field failed format validation.
    #[error("invalid {field} digest `{value}`: {reason}")]
    InvalidSha256Digest {
        field: &'static str,
        value: String,
        reason: String,
    },

    /// A target platform identifier failed `<os>/<arch>` validation.
    #[error("invalid target platform `{target}`: {reason}")]
    InvalidTargetPlatform { target: String, reason: String },

    /// A bound native lockfile path escaped the repository root or used invalid separators.
    #[error("invalid lockfile path `{path}`: {reason}")]
    InvalidLockfilePath { path: String, reason: String },

    /// A structured contract range failed invariant validation.
    #[error("invalid contract range for `{contract}`: {reason}")]
    InvalidContractRange { contract: String, reason: String },

    /// A manifest declares external requirements but `bsmr.lock` is missing.
    #[error(
        "component `{component}` declares contract requirements without `{lock_path}`; \
         run `bsmr deps update` to generate a verified lockfile"
    )]
    MissingLockfile {
        component: String,
        lock_path: String,
    },

    /// Two components claim to provide the same contract name.
    #[error("duplicate provider for contract `{contract}`: `{first}` and `{second}`")]
    DuplicateProvider {
        contract: String,
        first: String,
        second: String,
    },

    /// No component in the lock provides a required contract.
    #[error(
        "missing provider for contract `{contract}` (required {required_range} by {requirers})"
    )]
    MissingProvider {
        contract: String,
        required_range: String,
        requirers: String,
    },

    /// The intersection of all requirements for a contract is empty.
    #[error("empty requirement intersection for contract `{contract}`: {requirements}")]
    EmptyRangeIntersection {
        contract: String,
        requirements: String,
    },

    /// The locked provider version falls outside the required contract intersection.
    #[error(
        "incompatible version for contract `{contract}`: component `{provider}` provides {version}, \
         but consumers require {required_range} ({requirers})"
    )]
    IncompatibleVersion {
        contract: String,
        provider: String,
        version: u64,
        required_range: String,
        requirers: String,
    },

    /// A local manifest does not match its locked manifest digest or provided contracts.
    #[error("stale lock entry for component `{component}`: {reason}")]
    StaleManifest { component: String, reason: String },

    /// A committed native ecosystem lockfile on disk differs from `bsmr.lock`.
    #[error(
        "stale {ecosystem} lockfile `{path}` for component `{component}`: \
         locked `{expected}`, disk `{actual}`; run `bsmr deps update --refresh-lockfiles`"
    )]
    StaleLockfile {
        component: String,
        ecosystem: String,
        path: String,
        expected: String,
        actual: String,
    },

    /// A locked component violates a manifest provenance or patch policy.
    #[error("policy violation for component `{component}`: {reason}")]
    PolicyViolation { component: String, reason: String },

    /// No candidate in the catalog satisfies all contract requirements.
    #[error("no compatible candidate for component `{component}`:\n{details}")]
    NoCompatibleCandidate { component: String, details: String },

    /// A component requested for update or closure lookup does not exist.
    #[error("unknown component `{0}`")]
    UnknownComponent(String),

    /// Certification cannot proceed while any component uses an uncommitted local overlay.
    #[error("cannot certify release with uncommitted local overlays: {components}")]
    OverlayNotCertifiable { components: String },
}

/// Validates a canonical component name.
pub fn validate_component_name(name: &str) -> Result<(), DependencySetError> {
    if name.is_empty() {
        return Err(DependencySetError::InvalidComponentName {
            name: name.to_owned(),
            reason: "must not be empty".to_owned(),
        });
    }
    let bytes = name.as_bytes();
    if !bytes[0].is_ascii_lowercase() && !bytes[0].is_ascii_digit() {
        return Err(DependencySetError::InvalidComponentName {
            name: name.to_owned(),
            reason: "must start with a lowercase ASCII letter or digit".to_owned(),
        });
    }
    if !bytes
        .iter()
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-' || *b == b'_')
    {
        return Err(DependencySetError::InvalidComponentName {
            name: name.to_owned(),
            reason: "may contain only lowercase ASCII letters, digits, `-`, and `_`".to_owned(),
        });
    }
    Ok(())
}

/// Validates a globally stable dot-separated contract identifier.
pub fn validate_contract_name(name: &str) -> Result<(), DependencySetError> {
    if name.is_empty() {
        return Err(DependencySetError::InvalidContractName {
            name: name.to_owned(),
            reason: "must not be empty".to_owned(),
        });
    }
    for segment in name.split('.') {
        if segment.is_empty() {
            return Err(DependencySetError::InvalidContractName {
                name: name.to_owned(),
                reason: "must not contain empty dot-separated segments".to_owned(),
            });
        }
        let bytes = segment.as_bytes();
        if !bytes[0].is_ascii_lowercase() && !bytes[0].is_ascii_digit() {
            return Err(DependencySetError::InvalidContractName {
                name: name.to_owned(),
                reason: format!("segment `{segment}` must start with a lowercase letter or digit"),
            });
        }
        if !bytes
            .iter()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || *b == b'-' || *b == b'_')
        {
            return Err(DependencySetError::InvalidContractName {
                name: name.to_owned(),
                reason: format!("segment `{segment}` contains invalid characters"),
            });
        }
    }
    Ok(())
}

/// Validates a full 40-character lowercase hexadecimal Git commit SHA.
pub fn validate_commit_sha(revision: &str) -> Result<(), DependencySetError> {
    let valid = RawDigest::parse_sha1(revision.as_bytes())
        .is_ok_and(|parsed| parsed.to_string() == revision);
    if !valid {
        return Err(DependencySetError::InvalidCommitSha {
            revision: revision.to_owned(),
        });
    }
    Ok(())
}

/// Validates a `sha256:<64 lowercase hex>` content digest.
pub fn validate_sha256_digest(field: &'static str, value: &str) -> Result<(), DependencySetError> {
    let Some(hex_part) = value.strip_prefix("sha256:") else {
        return Err(DependencySetError::InvalidSha256Digest {
            field,
            value: value.to_owned(),
            reason: "must start with `sha256:`".to_owned(),
        });
    };
    let valid = RawDigest::parse_sha256(hex_part.as_bytes())
        .is_ok_and(|parsed| parsed.to_string() == hex_part);
    if !valid {
        return Err(DependencySetError::InvalidSha256Digest {
            field,
            value: value.to_owned(),
            reason: "must carry 64 lowercase hexadecimal characters after `sha256:`".to_owned(),
        });
    }
    Ok(())
}

/// Validates a target platform identifier of the form `<os>/<arch>`.
pub fn validate_target_platform(target: &str) -> Result<(), DependencySetError> {
    let Some((os, arch)) = target.split_once('/') else {
        return Err(DependencySetError::InvalidTargetPlatform {
            target: target.to_owned(),
            reason: "must have the form `<os>/<arch>`".to_owned(),
        });
    };
    if os.is_empty() || arch.is_empty() || arch.contains('/') {
        return Err(DependencySetError::InvalidTargetPlatform {
            target: target.to_owned(),
            reason: "must have the form `<os>/<arch>`".to_owned(),
        });
    }
    if !os
        .bytes()
        .chain(arch.bytes())
        .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
    {
        return Err(DependencySetError::InvalidTargetPlatform {
            target: target.to_owned(),
            reason: "contains invalid characters".to_owned(),
        });
    }
    Ok(())
}

/// Computes `sha256:<64 lowercase hex>` over `bytes`.
#[must_use]
pub fn sha256_prefixed(bytes: &[u8]) -> String {
    format!("sha256:{}", RawDigest::Sha256(Sha256::digest(bytes).into()))
}
