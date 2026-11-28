//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Evaluates ordered TOML candidate catalogs and executes explicit DependencySet lock updates.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde::Deserialize;
use serde::Serialize;

use super::ContractRange;
use super::DependencyLock;
use super::DependencySet;
use super::DependencySetError;
use super::LockedArtifact;
use super::LockedComponent;
use super::LockedLockfile;
use super::VerifiedResolution;
use super::sha256_prefixed;
use super::validate_component_name;
use super::validate_sha256_digest;
use super::verify;

/// One candidate entry in an immutable or authenticated TOML update catalog.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateEntry {
    /// Target component name.
    pub component: String,
    /// Strictly ordered release number. Git timestamps and hashes never rank candidates.
    pub release: u64,
    /// Source repository URL.
    pub repository: String,
    /// Full 40-character lowercase Git commit SHA.
    pub revision: String,
    /// Canonical source tree digest (`sha256:<64hex>`).
    pub tree: String,
    /// Canonical component manifest digest (`sha256:<64hex>`).
    pub manifest: String,
    /// Ordered patch blob digests (`sha256:<64hex>`).
    #[serde(default)]
    pub patches: Vec<String>,
    /// Exact contract versions provided by this candidate.
    #[serde(default)]
    pub provides: BTreeMap<String, u64>,
    /// Structured contract ranges required by this candidate.
    #[serde(default)]
    pub requires: BTreeMap<String, ContractRange>,
    /// Built artifacts published for this candidate.
    #[serde(default)]
    pub artifacts: Vec<LockedArtifact>,
    /// Native ecosystem lockfiles bound to this candidate.
    #[serde(default)]
    pub lockfiles: Vec<LockedLockfile>,
}

/// Immutable TOML candidate catalog consumed by `bsmr deps update`.
#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CandidateCatalog {
    /// Schema version. Must be `1`.
    pub schema: u32,
    /// Available release candidates.
    #[serde(default)]
    pub candidates: Vec<CandidateEntry>,
}

impl CandidateCatalog {
    /// Reads and parses a TOML catalog from disk and optionally verifies its exact SHA-256 digest.
    pub fn from_path(
        path: &Path,
        expected_digest: Option<&str>,
    ) -> Result<Self, DependencySetError> {
        let bytes = fs::read(path).map_err(|err| DependencySetError::Io {
            operation: "read candidate catalog",
            path: path.display().to_string(),
            message: err.to_string(),
        })?;
        if let Some(expected) = expected_digest {
            validate_sha256_digest("catalog_digest", expected)?;
            let actual = sha256_prefixed(&bytes);
            if actual != expected {
                return Err(DependencySetError::InvalidCatalog(format!(
                    "catalog digest mismatch: expected `{expected}`, got `{actual}`"
                )));
            }
        }
        let text = String::from_utf8(bytes)
            .map_err(|err| DependencySetError::InvalidCatalog(err.to_string()))?;
        Self::from_toml_str(&text)
    }

    /// Parses and validates a catalog from TOML text.
    pub fn from_toml_str(text: &str) -> Result<Self, DependencySetError> {
        let mut catalog: Self = toml::from_str(text)
            .map_err(|err| DependencySetError::InvalidCatalog(err.to_string()))?;
        catalog.validate()?;
        Ok(catalog)
    }

    /// Validates schema, release uniqueness per component, and candidate identities.
    pub fn validate(&mut self) -> Result<(), DependencySetError> {
        if self.schema != 1 {
            return Err(DependencySetError::InvalidCatalog(format!(
                "unsupported catalog schema {}; expected 1",
                self.schema
            )));
        }
        let mut seen_releases: BTreeSet<(String, u64)> = BTreeSet::new();
        for entry in &mut self.candidates {
            validate_component_name(&entry.component)?;
            if !seen_releases.insert((entry.component.clone(), entry.release)) {
                return Err(DependencySetError::InvalidCatalog(format!(
                    "duplicate release {} for component `{}`",
                    entry.release, entry.component
                )));
            }
            let mut locked = entry.to_locked_component();
            locked.validate_and_canonicalize()?;
            entry.requires = locked.requires;
            entry.artifacts = locked.artifacts;
        }
        Ok(())
    }
}

impl CandidateEntry {
    /// Converts this catalog entry into a [`LockedComponent`].
    #[must_use]
    pub fn to_locked_component(&self) -> LockedComponent {
        LockedComponent {
            name: self.component.clone(),
            repository: self.repository.clone(),
            revision: self.revision.clone(),
            tree: self.tree.clone(),
            manifest: self.manifest.clone(),
            patches: self.patches.clone(),
            overlay: None,
            provides: self.provides.clone(),
            requires: self.requires.clone(),
            artifacts: self.artifacts.clone(),
            lockfiles: self.lockfiles.clone(),
        }
    }
}

/// Explanation of why one catalog candidate was selected or rejected.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct CandidateDecision {
    /// Component name.
    pub component: String,
    /// Declared release number.
    pub release: u64,
    /// Candidate commit SHA.
    pub revision: String,
    /// Whether this candidate was chosen.
    pub selected: bool,
    /// Deciding contract name when rejected by a contract constraint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deciding_contract: Option<String>,
    /// Human-readable reason for selection or rejection.
    pub reason: String,
}

/// Update report for one component selected in `bsmr deps update`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ComponentUpdateReport {
    /// Component name.
    pub component: String,
    /// Previously locked commit revision, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_revision: Option<String>,
    /// Newly selected commit revision.
    pub selected_revision: String,
    /// Newly selected release number.
    pub selected_release: u64,
    /// Decisions recorded for every evaluated candidate in descending release order.
    pub decisions: Vec<CandidateDecision>,
}

/// Result of `update(dependency_set, lock, selected_components, catalog)`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct UpdateOutcome {
    /// Canonical updated lockfile.
    pub lock: DependencyLock,
    /// Verification result for the updated lockfile.
    pub verification: VerifiedResolution,
    /// Per-component selection and rejection reports.
    pub reports: Vec<ComponentUpdateReport>,
}

/// Explicitly updates the selected components in `lock` using an ordered candidate catalog.
///
/// Candidates are ordered strictly by their declared `release` version
/// descending. The newest candidate satisfying every consumer requirement and
/// manifest policy is chosen; rejected candidates record the deciding contract.
pub fn update(
    dependency_set: &DependencySet,
    lock: &DependencyLock,
    selected_components: &[String],
    catalog: &CandidateCatalog,
) -> Result<UpdateOutcome, DependencySetError> {
    let mut updated_lock = lock.clone();
    updated_lock.validate_and_canonicalize()?;
    let mut validated_catalog = catalog.clone();
    validated_catalog.validate()?;

    let targets: Vec<String> = if selected_components.is_empty() {
        let names: BTreeSet<String> = validated_catalog
            .candidates
            .iter()
            .map(|entry| entry.component.clone())
            .collect();
        if names.is_empty() {
            return Err(DependencySetError::InvalidCatalog(
                "catalog contains no candidates".to_owned(),
            ));
        }
        names.into_iter().collect()
    } else {
        for name in selected_components {
            validate_component_name(name)?;
        }
        selected_components.to_vec()
    };

    let mut reports = Vec::with_capacity(targets.len());
    for component_name in targets {
        let mut candidates: Vec<&CandidateEntry> = validated_catalog
            .candidates
            .iter()
            .filter(|c| c.component == component_name)
            .collect();
        if candidates.is_empty() {
            return Err(DependencySetError::NoCompatibleCandidate {
                component: component_name,
                details: "  catalog contains no candidates for this component".to_owned(),
            });
        }
        // Rank strictly by declared release version descending, never by Git hash or time.
        candidates.sort_by_key(|candidate| std::cmp::Reverse(candidate.release));

        let previous_revision = updated_lock
            .component(&component_name)
            .map(|c| c.revision.clone());

        let mut decisions = Vec::with_capacity(candidates.len());
        let mut chosen: Option<(&CandidateEntry, DependencyLock)> = None;

        for candidate in candidates {
            let mut trial_lock = updated_lock.clone();
            let locked = candidate.to_locked_component();
            if let Some(existing) = trial_lock.component_mut(&component_name) {
                *existing = locked;
            } else {
                trial_lock.components.push(locked);
            }

            if chosen.is_some() {
                decisions.push(CandidateDecision {
                    component: component_name.clone(),
                    release: candidate.release,
                    revision: candidate.revision.clone(),
                    selected: false,
                    deciding_contract: None,
                    reason: "skipped: newer compatible release already selected".to_owned(),
                });
                continue;
            }

            match check_candidate_compatibility(dependency_set, &trial_lock, &component_name) {
                Ok(()) => {
                    decisions.push(CandidateDecision {
                        component: component_name.clone(),
                        release: candidate.release,
                        revision: candidate.revision.clone(),
                        selected: true,
                        deciding_contract: None,
                        reason: format!("selected newest compatible release {}", candidate.release),
                    });
                    chosen = Some((candidate, trial_lock));
                }
                Err((deciding_contract, reason)) => {
                    decisions.push(CandidateDecision {
                        component: component_name.clone(),
                        release: candidate.release,
                        revision: candidate.revision.clone(),
                        selected: false,
                        deciding_contract,
                        reason,
                    });
                }
            }
        }

        let Some((winner, next_lock)) = chosen else {
            let details = decisions
                .iter()
                .map(|d| match &d.deciding_contract {
                    Some(contract) => format!(
                        "  release {} ({}): rejected by `{contract}`: {}",
                        d.release,
                        &d.revision[..12],
                        d.reason
                    ),
                    None => format!(
                        "  release {} ({}): rejected: {}",
                        d.release,
                        &d.revision[..12],
                        d.reason
                    ),
                })
                .collect::<Vec<_>>()
                .join("\n");
            return Err(DependencySetError::NoCompatibleCandidate {
                component: component_name,
                details,
            });
        };

        updated_lock = next_lock;
        updated_lock.validate_and_canonicalize()?;
        reports.push(ComponentUpdateReport {
            component: component_name,
            previous_revision,
            selected_revision: winner.revision.clone(),
            selected_release: winner.release,
            decisions,
        });
    }

    let verification = verify(dependency_set, &updated_lock)?;
    Ok(UpdateOutcome {
        lock: updated_lock,
        verification,
        reports,
    })
}

/// Evaluates a candidate lock during `update` or `explain`, extracting the deciding contract.
pub(super) fn check_candidate_compatibility(
    dependency_set: &DependencySet,
    trial_lock: &DependencyLock,
    candidate_component: &str,
) -> Result<(), (Option<String>, String)> {
    let err = match verify(dependency_set, trial_lock) {
        Ok(_) => return Ok(()),
        Err(err) => err,
    };

    let deciding_contract = match &err {
        DependencySetError::IncompatibleVersion { contract, .. }
        | DependencySetError::MissingProvider { contract, .. }
        | DependencySetError::DuplicateProvider { contract, .. }
        | DependencySetError::EmptyRangeIntersection { contract, .. }
        | DependencySetError::InvalidContractRange { contract, .. } => Some(contract.clone()),
        DependencySetError::StaleManifest { component, .. }
        | DependencySetError::PolicyViolation { component, .. }
            if component == candidate_component =>
        {
            None
        }
        _ => None,
    };

    Err((deciding_contract, err.to_string()))
}
