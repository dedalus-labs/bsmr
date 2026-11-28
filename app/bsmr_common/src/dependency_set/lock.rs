//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Parses, validates, canonicalizes, and hashes exact DependencyLock files (`bsmr.lock`).

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::collections::VecDeque;
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::Path;

use bsmr_fs::paths::forward_rel_path::ForwardRelativePath;
use serde::Deserialize;
use serde::Serialize;

use super::ContractRange;
use super::DEPENDENCY_LOCK_FORMAT;
use super::DependencyLockDigest;
use super::DependencySetError;
use super::sha256_prefixed;
use super::validate_commit_sha;
use super::validate_component_name;
use super::validate_contract_name;
use super::validate_sha256_digest;
use crate::cas_digest::CasDigestConfig;

/// Native package-manager ecosystem whose committed lockfile a component binds.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    Ord,
    PartialEq,
    PartialOrd,
    Deserialize,
    Serialize
)]
#[serde(rename_all = "lowercase")]
pub enum LockfileEcosystem {
    /// Cargo (`Cargo.lock`, `Cargo.toml`).
    Cargo,
    /// Go modules (`go.mod`, `go.sum`).
    Go,
    /// pnpm workspace (`pnpm-lock.yaml`, `pnpm-workspace.yaml`).
    Pnpm,
    /// Python PEP 751 / uv (`pylock.toml`, `uv.lock`, `pyproject.toml`).
    Python,
}

impl fmt::Display for LockfileEcosystem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Cargo => f.write_str("cargo"),
            Self::Go => f.write_str("go"),
            Self::Pnpm => f.write_str("pnpm"),
            Self::Python => f.write_str("python"),
        }
    }
}

/// One committed native ecosystem lockfile pinned by content digest on a component.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LockedLockfile {
    /// Native resolver ecosystem owning this lockfile.
    pub ecosystem: LockfileEcosystem,
    /// Repository-relative POSIX path to the committed lockfile.
    pub path: String,
    /// Exact SHA-256 content digest (`sha256:<64hex>`) of the lockfile bytes.
    pub digest: String,
}

/// One exact built artifact recorded on a locked component.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LockedArtifact {
    /// Logical artifact name unique within its component.
    pub name: String,
    /// Immutable content digest (`sha256:<64hex>`).
    pub digest: String,
    /// Optional immutable reference (for example `repo@sha256:<64hex>`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
}

/// One exact resolved component entry in `bsmr.lock`.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LockedComponent {
    /// Canonical component name.
    pub name: String,
    /// Source repository URL for provenance and review links.
    pub repository: String,
    /// Exact 40-character lowercase Git commit SHA.
    pub revision: String,
    /// Canonical source tree digest (`sha256:<64hex>`).
    pub tree: String,
    /// Canonical component manifest digest (`sha256:<64hex>`).
    pub manifest: String,
    /// Ordered patch blob digests (`sha256:<64hex>`). Order is semantic.
    #[serde(default)]
    pub patches: Vec<String>,
    /// Optional local development checkout overlay. Rejected during certification.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overlay: Option<String>,
    /// Exact contract versions provided by this component revision.
    #[serde(default)]
    pub provides: BTreeMap<String, u64>,
    /// Exact structured contract ranges required by this component revision.
    #[serde(default)]
    pub requires: BTreeMap<String, ContractRange>,
    /// Built artifacts produced by this component revision.
    #[serde(default)]
    pub artifacts: Vec<LockedArtifact>,
    /// Native ecosystem lockfiles bound to this component's closure.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub lockfiles: Vec<LockedLockfile>,
}

/// Canonical exact dependency resolution persisted in `bsmr.lock`.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DependencyLock {
    /// Schema version. Must be `1`.
    pub schema: u32,
    /// Locked components sorted canonically by `name`.
    #[serde(default)]
    pub components: Vec<LockedComponent>,
}

impl LockedComponent {
    /// Validates every identity, patch digest, contract entry, artifact, and lockfile on this component.
    pub fn validate_and_canonicalize(&mut self) -> Result<(), DependencySetError> {
        validate_component_name(&self.name)?;
        if self.repository.trim().is_empty() {
            return Err(DependencySetError::InvalidLock(format!(
                "component `{}` requires a non-empty repository URL",
                self.name
            )));
        }
        validate_commit_sha(&self.revision)?;
        validate_sha256_digest("tree", &self.tree)?;
        validate_sha256_digest("manifest", &self.manifest)?;

        // Patch order is semantic; validate each entry without reordering.
        for patch in &self.patches {
            validate_sha256_digest("patches", patch)?;
        }

        if let Some(overlay) = &self.overlay {
            if overlay.trim().is_empty() {
                return Err(DependencySetError::InvalidLock(format!(
                    "component `{}` overlay must not be empty when specified",
                    self.name
                )));
            }
        }

        for contract in self.provides.keys() {
            validate_contract_name(contract)?;
        }

        for (contract, range) in &mut self.requires {
            validate_contract_name(contract)?;
            range.validate_and_canonicalize(contract)?;
        }

        self.artifacts.sort_by(|a, b| a.name.cmp(&b.name));
        let mut seen_artifacts = BTreeSet::new();
        for artifact in &self.artifacts {
            if artifact.name.trim().is_empty() {
                return Err(DependencySetError::InvalidLock(format!(
                    "component `{}` contains an artifact with an empty name",
                    self.name
                )));
            }
            if !seen_artifacts.insert(artifact.name.as_str()) {
                return Err(DependencySetError::InvalidLock(format!(
                    "component `{}` contains duplicate artifact `{}`",
                    self.name, artifact.name
                )));
            }
            validate_sha256_digest("artifact.digest", &artifact.digest)?;
            if let Some(reference) = &artifact.reference {
                let expected_suffix = format!("@{}", artifact.digest);
                if !reference.ends_with(&expected_suffix)
                    || reference.len() <= expected_suffix.len()
                {
                    return Err(DependencySetError::InvalidLock(format!(
                        "component `{}` artifact `{}` reference `{reference}` must end with `{expected_suffix}`",
                        self.name, artifact.name
                    )));
                }
            }
        }

        self.lockfiles.sort_by(|a, b| a.path.cmp(&b.path));
        let mut seen_lockfiles = BTreeSet::new();
        for lockfile in &self.lockfiles {
            validate_relative_lockfile_path(&lockfile.path)?;
            if !seen_lockfiles.insert(lockfile.path.as_str()) {
                return Err(DependencySetError::InvalidLock(format!(
                    "component `{}` contains duplicate lockfile `{}`",
                    self.name, lockfile.path
                )));
            }
            validate_sha256_digest("lockfile.digest", &lockfile.digest)?;
        }

        Ok(())
    }
}

/// Rejects empty, absolute, backslash, and parent-traversal paths via [`ForwardRelativePath`].
fn validate_relative_lockfile_path(path: &str) -> Result<(), DependencySetError> {
    let parsed =
        ForwardRelativePath::new(path).map_err(|err| DependencySetError::InvalidLockfilePath {
            path: path.to_owned(),
            reason: err.to_string(),
        })?;
    if parsed.is_empty() {
        return Err(DependencySetError::InvalidLockfilePath {
            path: path.to_owned(),
            reason: "must not be empty".to_owned(),
        });
    }
    Ok(())
}

impl DependencyLock {
    /// Parses, validates, and canonicalizes a `bsmr.lock` from TOML text.
    pub fn from_toml_str(text: &str) -> Result<Self, DependencySetError> {
        let mut lock: Self =
            toml::from_str(text).map_err(|err| DependencySetError::InvalidLock(err.to_string()))?;
        lock.validate_and_canonicalize()?;
        Ok(lock)
    }

    /// Reads, parses, validates, and canonicalizes a `bsmr.lock` from disk.
    pub fn from_path(path: &Path) -> Result<Self, DependencySetError> {
        let text = fs::read_to_string(path).map_err(|err| DependencySetError::Io {
            operation: "read dependency lock",
            path: path.display().to_string(),
            message: err.to_string(),
        })?;
        Self::from_toml_str(&text)
    }

    /// Validates all components, rejects duplicate names, and sorts canonically.
    pub fn validate_and_canonicalize(&mut self) -> Result<(), DependencySetError> {
        if self.schema != 1 {
            return Err(DependencySetError::InvalidLock(format!(
                "unsupported schema version {}; expected 1",
                self.schema
            )));
        }
        for component in &mut self.components {
            component.validate_and_canonicalize()?;
        }
        self.components.sort_by(|a, b| a.name.cmp(&b.name));
        let mut seen = BTreeSet::new();
        for component in &self.components {
            if !seen.insert(component.name.as_str()) {
                return Err(DependencySetError::InvalidLock(format!(
                    "duplicate component `{}` in lockfile",
                    component.name
                )));
            }
        }
        Ok(())
    }

    /// Looks up a locked component by canonical name.
    #[must_use]
    pub fn component(&self, name: &str) -> Option<&LockedComponent> {
        self.components.iter().find(|c| c.name == name)
    }

    /// Returns a mutable reference to a locked component by canonical name.
    pub fn component_mut(&mut self, name: &str) -> Option<&mut LockedComponent> {
        self.components.iter_mut().find(|c| c.name == name)
    }

    /// Encodes the sorted lock graph with the `bsmr.dependency-lock.v1` domain prefix.
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, DependencySetError> {
        let mut normalized = self.clone();
        normalized.validate_and_canonicalize()?;
        let body = serde_json::to_vec(&normalized)
            .map_err(|err| DependencySetError::InvalidLock(err.to_string()))?;
        let mut bytes = Vec::with_capacity(DEPENDENCY_LOCK_FORMAT.len() + body.len());
        bytes.extend_from_slice(DEPENDENCY_LOCK_FORMAT);
        bytes.extend_from_slice(&body);
        Ok(bytes)
    }

    /// Formats this lockfile as deterministic, reviewable TOML.
    pub fn to_canonical_toml(&self) -> Result<String, DependencySetError> {
        let mut normalized = self.clone();
        normalized.validate_and_canonicalize()?;
        toml::to_string_pretty(&normalized)
            .map_err(|err| DependencySetError::InvalidLock(err.to_string()))
    }

    /// Computes the typed CAS digest under the repository's digest configuration.
    pub fn digest(
        &self,
        config: CasDigestConfig,
    ) -> Result<DependencyLockDigest, DependencySetError> {
        Ok(DependencyLockDigest::from_content(
            &self.canonical_bytes()?,
            config,
        ))
    }

    /// Computes the canonical `sha256:<64hex>` identity of this `DependencyLock`.
    pub fn sha256_digest(&self) -> Result<String, DependencySetError> {
        Ok(sha256_prefixed(&self.canonical_bytes()?))
    }

    /// Computes the content digest of only the components reachable from `root_component`.
    ///
    /// Reachability follows contract requirements to their providers. Mutating
    /// an unrelated component outside this closure leaves this digest unchanged.
    pub fn component_closure_digest(
        &self,
        root_component: &str,
    ) -> Result<String, DependencySetError> {
        let by_name: BTreeMap<&str, &LockedComponent> = self
            .components
            .iter()
            .map(|c| (c.name.as_str(), c))
            .collect();
        if !by_name.contains_key(root_component) {
            return Err(DependencySetError::UnknownComponent(
                root_component.to_owned(),
            ));
        }

        let mut provider_by_contract: BTreeMap<&str, &str> = BTreeMap::new();
        for component in &self.components {
            for contract in component.provides.keys() {
                provider_by_contract.insert(contract.as_str(), component.name.as_str());
            }
        }

        let mut visited: BTreeSet<&str> = BTreeSet::new();
        let mut queue: VecDeque<&str> = VecDeque::new();
        visited.insert(root_component);
        queue.push_back(root_component);

        while let Some(name) = queue.pop_front() {
            if let Some(component) = by_name.get(name) {
                for contract in component.requires.keys() {
                    if let Some(&provider) = provider_by_contract.get(contract.as_str()) {
                        if visited.insert(provider) {
                            queue.push_back(provider);
                        }
                    }
                }
            }
        }

        let closure: Vec<&LockedComponent> = visited
            .into_iter()
            .filter_map(|name| by_name.get(name).copied())
            .collect();
        let body = serde_json::to_vec(&closure)
            .map_err(|err| DependencySetError::InvalidLock(err.to_string()))?;
        let mut bytes = Vec::with_capacity(DEPENDENCY_LOCK_FORMAT.len() + body.len());
        bytes.extend_from_slice(DEPENDENCY_LOCK_FORMAT);
        bytes.extend_from_slice(&body);
        Ok(sha256_prefixed(&bytes))
    }

    /// Atomically writes canonical TOML bytes to `path` via temp file + sync + rename.
    pub fn write_atomic(&self, path: &Path) -> Result<(), DependencySetError> {
        let toml_text = self.to_canonical_toml()?;
        let parent = path.parent().ok_or_else(|| DependencySetError::Io {
            operation: "resolve parent directory",
            path: path.display().to_string(),
            message: "lock path must have a parent directory".to_owned(),
        })?;
        let mut temporary =
            tempfile::NamedTempFile::new_in(parent).map_err(|err| DependencySetError::Io {
                operation: "create temporary lockfile",
                path: path.display().to_string(),
                message: err.to_string(),
            })?;
        temporary
            .write_all(toml_text.as_bytes())
            .map_err(|err| DependencySetError::Io {
                operation: "write temporary lockfile",
                path: path.display().to_string(),
                message: err.to_string(),
            })?;
        temporary
            .as_file()
            .sync_all()
            .map_err(|err| DependencySetError::Io {
                operation: "sync temporary lockfile",
                path: path.display().to_string(),
                message: err.to_string(),
            })?;
        temporary
            .persist(path)
            .map_err(|err| DependencySetError::Io {
                operation: "replace lockfile",
                path: path.display().to_string(),
                message: err.to_string(),
            })?;
        Ok(())
    }
}
