//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Explains contract intersections, closure digests, conflicts, and catalog candidates.

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use serde::Serialize;

use super::CandidateCatalog;
use super::CandidateDecision;
use super::CandidateEntry;
use super::ContractRequirer;
use super::DependencyLock;
use super::DependencySet;
use super::DependencySetError;
use super::update::check_candidate_compatibility;
use super::verify;
use super::verify::collect_requirements;
use super::verify::intersect_requirers;

/// Diagnostic view of one contract used by `bsmr deps explain`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ContractExplanation {
    /// Contract name.
    pub contract: String,
    /// Intersected range across all requirers (or `empty` on conflict).
    pub required_range: String,
    /// All providers claiming this contract (`(component, version)`).
    pub providers: Vec<(String, u64)>,
    /// All requirers constraining this contract.
    pub requirers: Vec<ContractRequirer>,
    /// True if exactly one provider exists and satisfies the non-empty intersection.
    pub satisfied: bool,
    /// Conflict diagnostic when `satisfied` is false.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conflict: Option<String>,
}

/// Structured output for `bsmr deps explain`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ExplainReport {
    /// Canonical `bsmr.dependency-set.v1` SHA-256 digest.
    pub dependency_set_digest: String,
    /// Canonical `bsmr.dependency-lock.v1` SHA-256 digest.
    pub dependency_lock_digest: String,
    /// Overall verification status.
    pub valid: bool,
    /// Top-level verification error, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Contract-by-contract explanations.
    pub contracts: Vec<ContractExplanation>,
    /// Per-component reachable closure digests.
    pub component_closures: BTreeMap<String, String>,
    /// Components with uncommitted local overlays.
    pub overlays: Vec<String>,
    /// Optional candidate evaluations when a catalog is supplied.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub catalog_decisions: Vec<CandidateDecision>,
}

/// Produces a complete explanation of contracts, closures, conflicts, and optional catalog candidates.
pub fn explain(
    dependency_set: &DependencySet,
    lock: &DependencyLock,
    component_filter: Option<&str>,
    catalog: Option<&CandidateCatalog>,
) -> Result<ExplainReport, DependencySetError> {
    let mut normalized_lock = lock.clone();
    normalized_lock.validate_and_canonicalize()?;

    let dependency_set_digest = dependency_set.sha256_digest()?;
    let dependency_lock_digest = normalized_lock.sha256_digest()?;

    let verify_result = verify(dependency_set, &normalized_lock);
    let (valid, error) = match &verify_result {
        Ok(_) => (true, None),
        Err(err) => (false, Some(err.to_string())),
    };

    // Gather all providers (including duplicates so conflicts can be explained).
    let mut all_providers: BTreeMap<String, Vec<(String, u64)>> = BTreeMap::new();
    let mut seen_provider_pairs: BTreeSet<(String, String)> = BTreeSet::new();
    for component in &normalized_lock.components {
        for (contract, version) in &component.provides {
            if seen_provider_pairs.insert((contract.clone(), component.name.clone())) {
                all_providers
                    .entry(contract.clone())
                    .or_default()
                    .push((component.name.clone(), *version));
            }
        }
    }
    for manifest in dependency_set.manifests().values() {
        for (contract, version) in &manifest.provides {
            if seen_provider_pairs.insert((contract.clone(), manifest.name.clone())) {
                all_providers
                    .entry(contract.clone())
                    .or_default()
                    .push((manifest.name.clone(), *version));
            }
        }
    }

    let requirements = collect_requirements(dependency_set, &normalized_lock);
    let mut contract_names: BTreeSet<String> = BTreeSet::new();
    contract_names.extend(all_providers.keys().cloned());
    contract_names.extend(requirements.keys().cloned());

    let mut contracts = Vec::new();
    for contract in contract_names {
        let providers = all_providers.remove(&contract).unwrap_or_default();
        let requirers = requirements.get(&contract).cloned().unwrap_or_default();

        if let Some(filter) = component_filter {
            let touches_filter = providers.iter().any(|(c, _)| c == filter)
                || requirers.iter().any(|r| r.component == filter);
            if !touches_filter {
                continue;
            }
        }

        let (required_range_str, is_empty, parsed_intersection) = if requirers.is_empty() {
            (">=0".to_owned(), false, None)
        } else {
            let (intersection, _) = intersect_requirers(&contract, &requirers)?;
            let empty = intersection.is_empty();
            (intersection.to_string(), empty, Some(intersection))
        };

        let conflict = if providers.len() > 1 {
            Some(format!(
                "duplicate providers: {}",
                providers
                    .iter()
                    .map(|(c, v)| format!("{c}={v}"))
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        } else if is_empty {
            Some("empty requirement intersection".to_owned())
        } else if !requirers.is_empty() && providers.is_empty() {
            Some("missing provider".to_owned())
        } else if let (Some((_, version)), Some(intersection)) =
            (providers.first(), parsed_intersection.as_ref())
        {
            if !intersection.contains(*version) {
                Some(format!(
                    "provider version {version} is outside required range {required_range_str}"
                ))
            } else {
                None
            }
        } else {
            None
        };

        contracts.push(ContractExplanation {
            contract,
            required_range: required_range_str,
            providers,
            requirers,
            satisfied: conflict.is_none(),
            conflict,
        });
    }

    let mut component_closures = BTreeMap::new();
    let mut overlays = Vec::new();
    for component in &normalized_lock.components {
        if let Some(filter) = component_filter {
            if component.name != filter {
                continue;
            }
        }
        component_closures.insert(
            component.name.clone(),
            normalized_lock.component_closure_digest(&component.name)?,
        );
        if component.overlay.is_some() {
            overlays.push(component.name.clone());
        }
    }

    let mut catalog_decisions = Vec::new();
    if let Some(cat) = catalog {
        let mut entries: Vec<&CandidateEntry> = cat
            .candidates
            .iter()
            .filter(|entry| component_filter.is_none_or(|f| entry.component == f))
            .collect();
        entries.sort_by(|a, b| {
            a.component
                .cmp(&b.component)
                .then_with(|| b.release.cmp(&a.release))
        });
        for entry in entries {
            let mut trial_lock = normalized_lock.clone();
            let locked = entry.to_locked_component();
            if let Some(existing) = trial_lock.component_mut(&entry.component) {
                *existing = locked;
            } else {
                trial_lock.components.push(locked);
            }
            match check_candidate_compatibility(dependency_set, &trial_lock, &entry.component) {
                Ok(()) => catalog_decisions.push(CandidateDecision {
                    component: entry.component.clone(),
                    release: entry.release,
                    revision: entry.revision.clone(),
                    selected: true,
                    deciding_contract: None,
                    reason: "compatible with all contract requirements".to_owned(),
                }),
                Err((deciding_contract, reason)) => catalog_decisions.push(CandidateDecision {
                    component: entry.component.clone(),
                    release: entry.release,
                    revision: entry.revision.clone(),
                    selected: false,
                    deciding_contract,
                    reason,
                }),
            }
        }
    }

    Ok(ExplainReport {
        dependency_set_digest,
        dependency_lock_digest,
        valid,
        error,
        contracts,
        component_closures,
        overlays,
        catalog_decisions,
    })
}
