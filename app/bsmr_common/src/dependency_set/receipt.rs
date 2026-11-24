//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Generates and verifies immutable RFC 0004 release receipts.

use std::collections::BTreeSet;
use std::fs;
use std::io::Write;
use std::path::Path;

use serde::Deserialize;
use serde::Serialize;

use super::DependencyLock;
use super::DependencySet;
use super::DependencySetError;
use super::LockedComponent;
use super::RELEASE_RECEIPT_FORMAT;
use super::ReleaseReceiptDigest;
use super::sha256_prefixed;
use super::validate_commit_sha;
use super::validate_sha256_digest;
use super::verify;
use crate::cas_digest::CasDigestConfig;

/// Outcome of a build or test action recorded in a release receipt.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ActionOutcome {
    /// Required action succeeded.
    Passed,
    /// Required action failed.
    Failed,
}

/// One produced artifact attested by a release receipt.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReceiptArtifact {
    /// Logical target or service name (for example `api` or `worker`).
    pub target: String,
    /// Content digest (`sha256:<64hex>`).
    pub digest: String,
    /// Optional immutable OCI image reference (`repo@sha256:<64hex>`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    /// Optional immutable task-definition ARN or runtime descriptor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_definition: Option<String>,
    /// Optional build action digest that produced this artifact.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action_digest: Option<String>,
}

/// One required build or test action recorded in a release receipt.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ActionEvidence {
    /// Logical action or test suite name.
    pub name: String,
    /// Action digest (`sha256:<64hex>`).
    pub digest: String,
    /// Observed execution result.
    pub outcome: ActionOutcome,
}

/// Provenance identifying the exact build execution that certified a release.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BuilderProvenance {
    /// Builder principal or runner identity.
    pub identity: String,
    /// Workflow name or identifier.
    pub workflow: String,
    /// Execution run attempt (1-based).
    pub run_attempt: u64,
    /// Source Git commit SHA (40 lowercase hex).
    pub source_commit: String,
    /// Optional link to the build execution log.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_url: Option<String>,
}

/// Immutable release receipt binding DependencySet, exact lock, and produced artifacts.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReleaseReceipt {
    /// Schema version. Must be `1`.
    pub schema: u32,
    /// Canonical format identifier (`bsmr.release-receipt.v1`).
    pub format: String,
    /// Canonical `bsmr.dependency-set.v1` SHA-256 digest.
    pub dependency_set_digest: String,
    /// Canonical `bsmr.dependency-lock.v1` SHA-256 digest.
    pub dependency_lock_digest: String,
    /// SHA-256 digest of the canonical `bsmr.lock` TOML document bytes.
    pub lock_object_digest: String,
    /// Exact resolved components (never carrying local overlays).
    pub components: Vec<LockedComponent>,
    /// Produced artifacts sorted canonically by `target`.
    pub artifacts: Vec<ReceiptArtifact>,
    /// Required build and test action evidence sorted canonically by `name`.
    #[serde(default)]
    pub actions: Vec<ActionEvidence>,
    /// Builder provenance for the certification run.
    pub builder: BuilderProvenance,
    /// Optional external provenance or attestation references.
    #[serde(default)]
    pub attestations: Vec<String>,
}

impl ReleaseReceipt {
    /// Creates a verified, immutable release receipt from a DependencySet, lock, and build evidence.
    ///
    /// Fails closed if lock verification fails, any component carries an
    /// uncommitted local overlay, any required action failed, or any provenance
    /// policy requires attestations that are missing.
    pub fn create(
        dependency_set: &DependencySet,
        lock: &DependencyLock,
        mut artifacts: Vec<ReceiptArtifact>,
        mut actions: Vec<ActionEvidence>,
        builder: BuilderProvenance,
        attestations: Vec<String>,
    ) -> Result<Self, DependencySetError> {
        let verified = verify(dependency_set, lock)?;
        if verified.has_overlays() {
            return Err(DependencySetError::OverlayNotCertifiable {
                components: verified.overlays.join(", "),
            });
        }

        let require_attestation = dependency_set
            .manifests()
            .values()
            .any(|m| m.provenance.require_attestation);
        if require_attestation && attestations.is_empty() {
            return Err(DependencySetError::InvalidReceipt(
                "manifest provenance policy requires at least one attestation reference".to_owned(),
            ));
        }

        let mut normalized_lock = lock.clone();
        normalized_lock.validate_and_canonicalize()?;
        let lock_toml = normalized_lock.to_canonical_toml()?;
        let lock_object_digest = sha256_prefixed(lock_toml.as_bytes());

        artifacts.sort_by(|a, b| a.target.cmp(&b.target));
        actions.sort_by(|a, b| a.name.cmp(&b.name));

        let receipt = Self {
            schema: 1,
            format: "bsmr.release-receipt.v1".to_owned(),
            dependency_set_digest: verified.dependency_set_digest,
            dependency_lock_digest: verified.dependency_lock_digest,
            lock_object_digest,
            components: normalized_lock.components,
            artifacts,
            actions,
            builder,
            attestations,
        };
        receipt.validate()?;
        Ok(receipt)
    }

    /// Parses and validates a release receipt from JSON text.
    pub fn from_json_str(text: &str) -> Result<Self, DependencySetError> {
        let receipt: Self = serde_json::from_str(text)
            .map_err(|err| DependencySetError::InvalidReceipt(err.to_string()))?;
        receipt.validate()?;
        Ok(receipt)
    }

    /// Reads, parses, and validates a release receipt from disk.
    pub fn from_path(path: &Path) -> Result<Self, DependencySetError> {
        let text = fs::read_to_string(path).map_err(|err| DependencySetError::Io {
            operation: "read release receipt",
            path: path.display().to_string(),
            message: err.to_string(),
        })?;
        Self::from_json_str(&text)
    }

    /// Validates receipt schema, digests, component cleanliness, artifacts, actions, and builder provenance.
    pub fn validate(&self) -> Result<(), DependencySetError> {
        if self.schema != 1 {
            return Err(DependencySetError::InvalidReceipt(format!(
                "unsupported receipt schema {}; expected 1",
                self.schema
            )));
        }
        if self.format != "bsmr.release-receipt.v1" {
            return Err(DependencySetError::InvalidReceipt(format!(
                "unsupported receipt format `{}`",
                self.format
            )));
        }
        validate_sha256_digest("dependency_set_digest", &self.dependency_set_digest)?;
        validate_sha256_digest("dependency_lock_digest", &self.dependency_lock_digest)?;
        validate_sha256_digest("lock_object_digest", &self.lock_object_digest)?;

        let reconstructed_lock = DependencyLock {
            schema: 1,
            components: self.components.clone(),
        };
        let actual_lock_digest = reconstructed_lock.sha256_digest()?;
        if actual_lock_digest != self.dependency_lock_digest {
            return Err(DependencySetError::InvalidReceipt(format!(
                "components hash `{actual_lock_digest}` does not match dependency_lock_digest `{}`",
                self.dependency_lock_digest
            )));
        }
        let actual_lock_obj = sha256_prefixed(reconstructed_lock.to_canonical_toml()?.as_bytes());
        if actual_lock_obj != self.lock_object_digest {
            return Err(DependencySetError::InvalidReceipt(format!(
                "canonical lock object hash `{actual_lock_obj}` does not match lock_object_digest `{}`",
                self.lock_object_digest
            )));
        }

        let overlay_names: Vec<&str> = self
            .components
            .iter()
            .filter(|c| c.overlay.is_some())
            .map(|c| c.name.as_str())
            .collect();
        if !overlay_names.is_empty() {
            return Err(DependencySetError::OverlayNotCertifiable {
                components: overlay_names.join(", "),
            });
        }

        if self.artifacts.is_empty() {
            return Err(DependencySetError::InvalidReceipt(
                "release receipt must attest at least one produced artifact".to_owned(),
            ));
        }
        let mut seen_targets = BTreeSet::new();
        for artifact in &self.artifacts {
            if artifact.target.trim().is_empty() {
                return Err(DependencySetError::InvalidReceipt(
                    "artifact target must not be empty".to_owned(),
                ));
            }
            if !seen_targets.insert(artifact.target.as_str()) {
                return Err(DependencySetError::InvalidReceipt(format!(
                    "duplicate artifact target `{}`",
                    artifact.target
                )));
            }
            validate_sha256_digest("artifact.digest", &artifact.digest)?;
            if let Some(image) = &artifact.image {
                let expected_suffix = format!("@{}", artifact.digest);
                if !image.ends_with(&expected_suffix) || image.len() <= expected_suffix.len() {
                    return Err(DependencySetError::InvalidReceipt(format!(
                        "artifact `{}` image `{image}` must end with `{expected_suffix}`",
                        artifact.target
                    )));
                }
            }
            if let Some(action_digest) = &artifact.action_digest {
                validate_sha256_digest("artifact.action_digest", action_digest)?;
            }
        }

        for action in &self.actions {
            if action.name.trim().is_empty() {
                return Err(DependencySetError::InvalidReceipt(
                    "action evidence name must not be empty".to_owned(),
                ));
            }
            validate_sha256_digest("action.digest", &action.digest)?;
            if action.outcome != ActionOutcome::Passed {
                return Err(DependencySetError::InvalidReceipt(format!(
                    "required action `{}` did not pass",
                    action.name
                )));
            }
        }

        if self.builder.identity.trim().is_empty() {
            return Err(DependencySetError::InvalidReceipt(
                "builder.identity must not be empty".to_owned(),
            ));
        }
        if self.builder.workflow.trim().is_empty() {
            return Err(DependencySetError::InvalidReceipt(
                "builder.workflow must not be empty".to_owned(),
            ));
        }
        if self.builder.run_attempt == 0 {
            return Err(DependencySetError::InvalidReceipt(
                "builder.run_attempt must be >= 1".to_owned(),
            ));
        }
        validate_commit_sha(&self.builder.source_commit)?;

        for attestation in &self.attestations {
            if attestation.trim().is_empty() {
                return Err(DependencySetError::InvalidReceipt(
                    "attestation reference must not be empty".to_owned(),
                ));
            }
        }

        Ok(())
    }

    /// Verifies this receipt against optional expected DependencySet and DependencyLock digests.
    pub fn verify(
        &self,
        expected_dependency_set: Option<&str>,
        expected_dependency_lock: Option<&str>,
    ) -> Result<(), DependencySetError> {
        self.validate()?;
        if let Some(expected) = expected_dependency_set {
            if self.dependency_set_digest != expected {
                return Err(DependencySetError::InvalidReceipt(format!(
                    "receipt dependency_set_digest `{}` does not match expected `{expected}`",
                    self.dependency_set_digest
                )));
            }
        }
        if let Some(expected) = expected_dependency_lock {
            if self.dependency_lock_digest != expected {
                return Err(DependencySetError::InvalidReceipt(format!(
                    "receipt dependency_lock_digest `{}` does not match expected `{expected}`",
                    self.dependency_lock_digest
                )));
            }
        }
        Ok(())
    }

    /// Encodes the receipt with the `bsmr.release-receipt.v1` CAS domain prefix.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, DependencySetError> {
        self.validate()?;
        let body = serde_json::to_vec(self)
            .map_err(|err| DependencySetError::InvalidReceipt(err.to_string()))?;
        let mut bytes = Vec::with_capacity(RELEASE_RECEIPT_FORMAT.len() + body.len());
        bytes.extend_from_slice(RELEASE_RECEIPT_FORMAT);
        bytes.extend_from_slice(&body);
        Ok(bytes)
    }

    /// Serializes this receipt as deterministic pretty JSON with a trailing newline.
    pub fn to_canonical_json(&self) -> Result<String, DependencySetError> {
        self.validate()?;
        let mut json = serde_json::to_string_pretty(self)
            .map_err(|err| DependencySetError::InvalidReceipt(err.to_string()))?;
        json.push('\n');
        Ok(json)
    }

    /// Computes the typed CAS digest under the repository's digest configuration.
    pub fn digest(
        &self,
        config: CasDigestConfig,
    ) -> Result<ReleaseReceiptDigest, DependencySetError> {
        Ok(ReleaseReceiptDigest::from_content(
            &self.canonical_bytes()?,
            config,
        ))
    }

    /// Computes the canonical `sha256:<64hex>` identity of this release receipt.
    pub fn sha256_digest(&self) -> Result<String, DependencySetError> {
        Ok(sha256_prefixed(&self.canonical_bytes()?))
    }

    /// Publishes receipt JSON bytes once without ever overwriting an existing file.
    pub fn write_noclobber(&self, path: &Path) -> Result<(), DependencySetError> {
        let json = self.to_canonical_json()?;
        let parent = path.parent().ok_or_else(|| DependencySetError::Io {
            operation: "resolve parent directory",
            path: path.display().to_string(),
            message: "receipt path must have a parent directory".to_owned(),
        })?;
        let mut temporary =
            tempfile::NamedTempFile::new_in(parent).map_err(|err| DependencySetError::Io {
                operation: "create temporary receipt",
                path: path.display().to_string(),
                message: err.to_string(),
            })?;
        temporary
            .write_all(json.as_bytes())
            .map_err(|err| DependencySetError::Io {
                operation: "write temporary receipt",
                path: path.display().to_string(),
                message: err.to_string(),
            })?;
        temporary
            .as_file()
            .sync_all()
            .map_err(|err| DependencySetError::Io {
                operation: "sync temporary receipt",
                path: path.display().to_string(),
                message: err.to_string(),
            })?;
        temporary
            .persist_noclobber(path)
            .map_err(|err| DependencySetError::Io {
                operation: "retain immutable receipt",
                path: path.display().to_string(),
                message: err.to_string(),
            })?;
        Ok(())
    }
}
