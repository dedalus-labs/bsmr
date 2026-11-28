//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Parses, validates, and canonically encodes DependencySet component manifests.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde::Deserialize;
use serde::Serialize;

use super::ContractRange;
use super::DEPENDENCY_SET_FORMAT;
use super::DependencySetDigest;
use super::DependencySetError;
use super::sha256_prefixed;
use super::validate_component_name;
use super::validate_contract_name;
use super::validate_sha256_digest;
use super::validate_target_platform;
use crate::cas_digest::CasDigestConfig;

/// Patch acceptance policy declared by a component manifest.
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PatchPolicy {
    /// Allowed patch blob digests (`sha256:...`). When empty, any valid digest is permitted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed: Vec<String>,
}

/// Provenance policy declared by a component manifest.
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProvenancePolicy {
    /// Allowed repository URL prefixes. When empty, any non-empty repository URL is permitted.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub allowed_repositories: Vec<String>,
    /// Require attestation references when producing a release receipt.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub require_attestation: bool,
}

/// One component rule declaration parsed from `bsmr.component.toml`.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ComponentManifest {
    /// Schema version. Must be `1`.
    pub schema: u32,
    /// Globally unique canonical component name.
    pub name: String,
    /// Named integer contract versions this component provides.
    #[serde(default)]
    pub provides: BTreeMap<String, u64>,
    /// Named structured contract version ranges this component requires.
    #[serde(default)]
    pub requires: BTreeMap<String, ContractRange>,
    /// Target platforms supported by this component, such as `linux/amd64`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub targets: Vec<String>,
    /// Allowed patch policy for external components.
    #[serde(default, skip_serializing_if = "PatchPolicy::is_default")]
    pub patches: PatchPolicy,
    /// Provenance constraints for locked components and receipts.
    #[serde(default, skip_serializing_if = "ProvenancePolicy::is_default")]
    pub provenance: ProvenancePolicy,
    /// Optional catalog reference where update candidates may be discovered.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub catalog: Option<String>,
}

impl PatchPolicy {
    /// Returns true when no patch constraints are configured.
    fn is_default(&self) -> bool {
        self.allowed.is_empty()
    }
}

impl ProvenancePolicy {
    /// Returns true when no provenance constraints are configured.
    fn is_default(&self) -> bool {
        self.allowed_repositories.is_empty() && !self.require_attestation
    }
}

impl ComponentManifest {
    /// Parses and validates a component manifest from TOML text.
    pub fn from_toml_str(text: &str) -> Result<Self, DependencySetError> {
        let mut manifest: Self = toml::from_str(text)
            .map_err(|err| DependencySetError::InvalidManifest(err.to_string()))?;
        manifest.validate_and_canonicalize()?;
        Ok(manifest)
    }

    /// Reads, parses, and validates a component manifest from disk.
    pub fn from_path(path: &Path) -> Result<Self, DependencySetError> {
        let text = fs::read_to_string(path).map_err(|err| DependencySetError::Io {
            operation: "read component manifest",
            path: path.display().to_string(),
            message: err.to_string(),
        })?;
        Self::from_toml_str(&text)
    }

    /// Validates all schema, naming, range, target, and policy invariants in place.
    pub fn validate_and_canonicalize(&mut self) -> Result<(), DependencySetError> {
        if self.schema != 1 {
            return Err(DependencySetError::InvalidManifest(format!(
                "unsupported schema version {}; expected 1",
                self.schema
            )));
        }
        validate_component_name(&self.name)?;

        for contract in self.provides.keys() {
            validate_contract_name(contract)?;
        }

        for (contract, range) in &mut self.requires {
            validate_contract_name(contract)?;
            range.validate_and_canonicalize(contract)?;
        }

        let mut unique_targets = BTreeSet::new();
        for target in &self.targets {
            validate_target_platform(target)?;
            unique_targets.insert(target.clone());
        }
        self.targets = unique_targets.into_iter().collect();

        let mut unique_patches = BTreeSet::new();
        for digest in &self.patches.allowed {
            validate_sha256_digest("patches.allowed", digest)?;
            unique_patches.insert(digest.clone());
        }
        self.patches.allowed = unique_patches.into_iter().collect();

        let mut unique_repos = BTreeSet::new();
        for repo in &self.provenance.allowed_repositories {
            if repo.trim().is_empty() {
                return Err(DependencySetError::InvalidManifest(
                    "provenance.allowed_repositories entries must not be empty".to_owned(),
                ));
            }
            unique_repos.insert(repo.trim().to_owned());
        }
        self.provenance.allowed_repositories = unique_repos.into_iter().collect();

        if let Some(catalog) = &self.catalog {
            if catalog.trim().is_empty() {
                return Err(DependencySetError::InvalidManifest(
                    "catalog must not be empty when specified".to_owned(),
                ));
            }
        }

        Ok(())
    }

    /// Serializes this manifest into deterministic canonical JSON bytes.
    pub fn canonical_json_bytes(&self) -> Result<Vec<u8>, DependencySetError> {
        let mut normalized = self.clone();
        normalized.validate_and_canonicalize()?;
        serde_json::to_vec(&normalized)
            .map_err(|err| DependencySetError::InvalidManifest(err.to_string()))
    }

    /// Computes the canonical `sha256:<64hex>` identity of this manifest.
    pub fn sha256_digest(&self) -> Result<String, DependencySetError> {
        Ok(sha256_prefixed(&self.canonical_json_bytes()?))
    }
}

/// The rooted, canonically sorted rule graph of component manifests.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DependencySet {
    manifests: BTreeMap<String, ComponentManifest>,
}

impl DependencySet {
    /// Builds a validated `DependencySet` from one or more component manifests.
    pub fn new(
        manifests: impl IntoIterator<Item = ComponentManifest>,
    ) -> Result<Self, DependencySetError> {
        let mut by_name = BTreeMap::new();
        for mut manifest in manifests {
            manifest.validate_and_canonicalize()?;
            if by_name.contains_key(&manifest.name) {
                return Err(DependencySetError::InvalidManifest(format!(
                    "duplicate component manifest `{}`",
                    manifest.name
                )));
            }
            by_name.insert(manifest.name.clone(), manifest);
        }
        Ok(Self { manifests: by_name })
    }

    /// Builds a `DependencySet` containing a single validated root manifest.
    pub fn from_manifest(manifest: ComponentManifest) -> Result<Self, DependencySetError> {
        Self::new(std::iter::once(manifest))
    }

    /// Returns the sorted map of component manifests by component name.
    #[must_use]
    pub fn manifests(&self) -> &BTreeMap<String, ComponentManifest> {
        &self.manifests
    }

    /// Returns true if no manifests are present.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.manifests.is_empty()
    }

    /// Encodes the sorted rule graph with the `bsmr.dependency-set.v1` domain prefix.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, DependencySetError> {
        let body = serde_json::to_vec(&self.manifests)
            .map_err(|err| DependencySetError::InvalidManifest(err.to_string()))?;
        let mut bytes = Vec::with_capacity(DEPENDENCY_SET_FORMAT.len() + body.len());
        bytes.extend_from_slice(DEPENDENCY_SET_FORMAT);
        bytes.extend_from_slice(&body);
        Ok(bytes)
    }

    /// Computes the typed CAS digest under the repository's digest configuration.
    pub fn digest(
        &self,
        config: CasDigestConfig,
    ) -> Result<DependencySetDigest, DependencySetError> {
        Ok(DependencySetDigest::from_content(
            &self.canonical_bytes()?,
            config,
        ))
    }

    /// Computes the canonical `sha256:<64hex>` identity of this `DependencySet`.
    pub fn sha256_digest(&self) -> Result<String, DependencySetError> {
        Ok(sha256_prefixed(&self.canonical_bytes()?))
    }
}
