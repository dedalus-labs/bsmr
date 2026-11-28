//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies DependencySet locks offline, checks bound ecosystem lockfiles, and gates builds.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde::Deserialize;
use serde::Serialize;
use yaml_rust2::YamlLoader;

use super::COMPONENT_MANIFEST_FILE;
use super::ComponentManifest;
use super::ContractRange;
use super::ContractRangeSet;
use super::DEPENDENCY_LOCK_FILE;
use super::DependencyLock;
use super::DependencySet;
use super::DependencySetError;
use super::LockfileEcosystem;
use super::sha256_prefixed;

/// One component's requirement on a named contract.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ContractRequirer {
    /// Name of the requiring component.
    pub component: String,
    /// Structured contract range required by that component.
    pub range: ContractRange,
}

/// Verified contract assignment in a resolved DependencySet.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ContractResolution {
    /// Contract name.
    pub contract: String,
    /// Component providing the contract.
    pub provider: String,
    /// Exact integer contract version provided.
    pub version: u64,
    /// Intersection of every consumer requirement on this contract.
    pub required_range: String,
    /// Every component that requires this contract.
    pub requirers: Vec<ContractRequirer>,
}

/// Complete output of offline `verify(dependency_set, lock)`.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct VerifiedResolution {
    /// Canonical `bsmr.dependency-set.v1` SHA-256 digest.
    pub dependency_set_digest: String,
    /// Canonical `bsmr.dependency-lock.v1` SHA-256 digest.
    pub dependency_lock_digest: String,
    /// Verified contract assignments keyed by contract name.
    pub contracts: BTreeMap<String, ContractResolution>,
    /// Per-component reachable closure digests keyed by component name.
    pub component_closures: BTreeMap<String, String>,
    /// Component names currently carrying uncommitted local overlays.
    pub overlays: Vec<String>,
}

impl VerifiedResolution {
    /// Returns true if any locked component carries an uncommitted local overlay.
    #[must_use]
    pub fn has_overlays(&self) -> bool {
        !self.overlays.is_empty()
    }
}

/// Verifies that `lock` is a valid, compatible exact resolution of `dependency_set`.
///
/// Performs no network access and no candidate discovery. Fails closed on any
/// invalid identity, stale manifest digest, policy violation, duplicate or
/// missing provider, empty range intersection, or out-of-range contract version.
pub fn verify(
    dependency_set: &DependencySet,
    lock: &DependencyLock,
) -> Result<VerifiedResolution, DependencySetError> {
    verify_in_root(dependency_set, lock, None)
}

/// Verifies `lock` against `dependency_set` and, when `root` is provided, checks
/// every bound native ecosystem lockfile on disk against its pinned SHA-256 digest.
pub fn verify_in_root(
    dependency_set: &DependencySet,
    lock: &DependencyLock,
    root: Option<&Path>,
) -> Result<VerifiedResolution, DependencySetError> {
    let mut lock = lock.clone();
    lock.validate_and_canonicalize()?;

    let dependency_set_digest = dependency_set.sha256_digest()?;
    let dependency_lock_digest = lock.sha256_digest()?;

    check_manifest_and_policy_consistency(dependency_set, &lock)?;
    if let Some(project_root) = root {
        verify_bound_lockfiles(&lock, project_root)?;
    }

    let providers = collect_providers(dependency_set, &lock)?;
    let requirements = collect_requirements(dependency_set, &lock);

    let mut resolved_contracts = BTreeMap::new();
    for (contract, requirers) in requirements {
        let (intersection, formatted_reqs) = intersect_requirers(&contract, &requirers)?;
        if intersection.is_empty() {
            return Err(DependencySetError::EmptyRangeIntersection {
                contract,
                requirements: formatted_reqs,
            });
        }
        let Some((provider, version)) = providers.get(&contract) else {
            return Err(DependencySetError::MissingProvider {
                contract,
                required_range: intersection.to_string(),
                requirers: formatted_reqs,
            });
        };
        if !intersection.contains(*version) {
            return Err(DependencySetError::IncompatibleVersion {
                contract,
                provider: provider.clone(),
                version: *version,
                required_range: intersection.to_string(),
                requirers: formatted_reqs,
            });
        }
        resolved_contracts.insert(
            contract.clone(),
            ContractResolution {
                contract,
                provider: provider.clone(),
                version: *version,
                required_range: intersection.to_string(),
                requirers,
            },
        );
    }

    let mut component_closures = BTreeMap::new();
    let mut overlays = Vec::new();
    for component in &lock.components {
        component_closures.insert(
            component.name.clone(),
            lock.component_closure_digest(&component.name)?,
        );
        if component.overlay.is_some() {
            overlays.push(component.name.clone());
        }
    }

    Ok(VerifiedResolution {
        dependency_set_digest,
        dependency_lock_digest,
        contracts: resolved_contracts,
        component_closures,
        overlays,
    })
}

/// Loads `bsmr.component.toml` and `bsmr.lock` from a project root or explicit paths.
pub fn load_project_state(
    root: &Path,
    manifest_path: Option<&Path>,
    lock_path: Option<&Path>,
) -> Result<(DependencySet, DependencyLock), DependencySetError> {
    let resolved_manifest = manifest_path
        .map(Path::to_path_buf)
        .unwrap_or_else(|| root.join(COMPONENT_MANIFEST_FILE));
    let resolved_lock = lock_path
        .map(Path::to_path_buf)
        .unwrap_or_else(|| root.join(DEPENDENCY_LOCK_FILE));

    let dependency_set = if resolved_manifest.is_file() {
        DependencySet::from_manifest(ComponentManifest::from_path(&resolved_manifest)?)?
    } else if manifest_path.is_some() {
        return Err(DependencySetError::Io {
            operation: "read component manifest",
            path: resolved_manifest.display().to_string(),
            message: "file does not exist".to_owned(),
        });
    } else {
        DependencySet::default()
    };

    if !resolved_lock.is_file() {
        if let Some(manifest) = dependency_set.manifests().values().next() {
            return Err(DependencySetError::MissingLockfile {
                component: manifest.name.clone(),
                lock_path: resolved_lock.display().to_string(),
            });
        }
        return Err(DependencySetError::Io {
            operation: "read dependency lock",
            path: resolved_lock.display().to_string(),
            message: "file does not exist".to_owned(),
        });
    }

    let lock = DependencyLock::from_path(&resolved_lock)?;
    Ok((dependency_set, lock))
}

/// Verifies a repository's DependencySet lock before build analysis.
///
/// Projects without `bsmr.component.toml` and `bsmr.lock` return `Ok(None)`
/// immediately with two stat calls. Projects with a lockfile, or with a
/// manifest declaring external requirements, fail closed before analysis if the
/// lockfile is missing, stale, or incompatible.
pub fn verify_project_before_build(
    root: &Path,
) -> Result<Option<VerifiedResolution>, DependencySetError> {
    let manifest_path = root.join(COMPONENT_MANIFEST_FILE);
    let lock_path = root.join(DEPENDENCY_LOCK_FILE);

    let has_manifest = manifest_path.is_file();
    let has_lock = lock_path.is_file();
    if !has_manifest && !has_lock {
        return Ok(None);
    }

    let dependency_set = if has_manifest {
        let manifest = ComponentManifest::from_path(&manifest_path)?;
        if !has_lock {
            if manifest.requires.is_empty() {
                return Ok(None);
            }
            return Err(DependencySetError::MissingLockfile {
                component: manifest.name,
                lock_path: lock_path.display().to_string(),
            });
        }
        DependencySet::from_manifest(manifest)?
    } else {
        DependencySet::default()
    };

    let lock = DependencyLock::from_path(&lock_path)?;
    Ok(Some(verify_in_root(&dependency_set, &lock, Some(root))?))
}

/// Rehashes committed native ecosystem lockfiles on disk for selected components (or all).
pub fn refresh_lockfiles(
    dependency_set: &DependencySet,
    lock: &DependencyLock,
    root: &Path,
    selected_components: &[String],
) -> Result<(DependencyLock, VerifiedResolution, usize), DependencySetError> {
    let mut updated = lock.clone();
    updated.validate_and_canonicalize()?;

    let filter: BTreeSet<&str> = selected_components.iter().map(String::as_str).collect();
    for name in &filter {
        if updated.component(name).is_none() {
            return Err(DependencySetError::UnknownComponent((*name).to_owned()));
        }
    }

    let mut refreshed = 0usize;
    for component in &mut updated.components {
        if !filter.is_empty() && !filter.contains(component.name.as_str()) {
            continue;
        }
        for lockfile in &mut component.lockfiles {
            let full_path = root.join(&lockfile.path);
            let bytes = fs::read(&full_path).map_err(|err| DependencySetError::Io {
                operation: "read native lockfile",
                path: full_path.display().to_string(),
                message: err.to_string(),
            })?;
            validate_native_lockfile_syntax(lockfile.ecosystem, &lockfile.path, &bytes)?;
            let digest = sha256_prefixed(&bytes);
            if lockfile.digest != digest {
                lockfile.digest = digest;
                refreshed += 1;
            }
        }
    }

    updated.validate_and_canonicalize()?;
    let verified = verify_in_root(dependency_set, &updated, Some(root))?;
    Ok((updated, verified, refreshed))
}

/// Minimal typed Cargo.lock header deserialized by `toml`.
#[derive(Deserialize)]
struct CargoLockHeader {
    #[allow(dead_code)]
    version: Option<u32>,
    #[serde(default)]
    #[allow(dead_code)]
    package: Vec<toml::Table>,
}

/// Validates a bound native lockfile using typed ecosystem parsers (`toml`, `yaml_rust2`).
fn validate_native_lockfile_syntax(
    ecosystem: LockfileEcosystem,
    path: &str,
    bytes: &[u8],
) -> Result<(), DependencySetError> {
    let text = std::str::from_utf8(bytes).map_err(|err| {
        DependencySetError::InvalidLock(format!(
            "{ecosystem} lockfile `{path}` is not valid UTF-8: {err}"
        ))
    })?;
    match ecosystem {
        LockfileEcosystem::Cargo => {
            toml::from_str::<CargoLockHeader>(text).map_err(|err| {
                DependencySetError::InvalidLock(format!("invalid cargo lockfile `{path}`: {err}"))
            })?;
        }
        LockfileEcosystem::Pnpm => {
            let docs = YamlLoader::load_from_str(text).map_err(|err| {
                DependencySetError::InvalidLock(format!("invalid pnpm lockfile `{path}`: {err}"))
            })?;
            if docs.len() != 1 || docs[0].as_hash().is_none() {
                return Err(DependencySetError::InvalidLock(format!(
                    "invalid pnpm lockfile `{path}`: expected a single YAML mapping document"
                )));
            }
        }
        LockfileEcosystem::Python => {
            toml::from_str::<toml::Table>(text).map_err(|err| {
                DependencySetError::InvalidLock(format!("invalid python lockfile `{path}`: {err}"))
            })?;
        }
        LockfileEcosystem::Go => {}
    }
    Ok(())
}

/// Verifies every bound native ecosystem lockfile on disk against its syntax and pinned digest.
fn verify_bound_lockfiles(lock: &DependencyLock, root: &Path) -> Result<(), DependencySetError> {
    for component in &lock.components {
        for lockfile in &component.lockfiles {
            let full_path = root.join(&lockfile.path);
            let bytes = fs::read(&full_path).map_err(|err| DependencySetError::Io {
                operation: "read native lockfile",
                path: full_path.display().to_string(),
                message: err.to_string(),
            })?;
            validate_native_lockfile_syntax(lockfile.ecosystem, &lockfile.path, &bytes)?;
            let actual = sha256_prefixed(&bytes);
            if actual != lockfile.digest {
                return Err(DependencySetError::StaleLockfile {
                    component: component.name.clone(),
                    ecosystem: lockfile.ecosystem.to_string(),
                    path: lockfile.path.clone(),
                    expected: lockfile.digest.clone(),
                    actual,
                });
            }
        }
    }
    Ok(())
}

/// Checks manifest digests, provided contracts, and provenance/patch policies against the lock.
fn check_manifest_and_policy_consistency(
    dependency_set: &DependencySet,
    lock: &DependencyLock,
) -> Result<(), DependencySetError> {
    for manifest in dependency_set.manifests().values() {
        if let Some(locked) = lock.component(&manifest.name) {
            let expected_digest = manifest.sha256_digest()?;
            if locked.manifest != expected_digest {
                return Err(DependencySetError::StaleManifest {
                    component: manifest.name.clone(),
                    reason: format!(
                        "locked manifest digest `{}` does not match current manifest `{expected_digest}`",
                        locked.manifest
                    ),
                });
            }
            if locked.provides != manifest.provides {
                return Err(DependencySetError::StaleManifest {
                    component: manifest.name.clone(),
                    reason: "locked `provides` differs from component manifest".to_owned(),
                });
            }
        }

        if !manifest.provenance.allowed_repositories.is_empty() {
            for locked in &lock.components {
                let allowed = manifest
                    .provenance
                    .allowed_repositories
                    .iter()
                    .any(|prefix| locked.repository.starts_with(prefix));
                if !allowed {
                    return Err(DependencySetError::PolicyViolation {
                        component: locked.name.clone(),
                        reason: format!(
                            "repository `{}` is not permitted by `{}` provenance policy",
                            locked.repository, manifest.name
                        ),
                    });
                }
            }
        }

        if !manifest.patches.allowed.is_empty() {
            for locked in &lock.components {
                for patch in &locked.patches {
                    if !manifest.patches.allowed.contains(patch) {
                        return Err(DependencySetError::PolicyViolation {
                            component: locked.name.clone(),
                            reason: format!(
                                "patch `{patch}` is not in `{}` patches.allowed",
                                manifest.name
                            ),
                        });
                    }
                }
            }
        }
    }
    Ok(())
}

/// Collects unique contract providers across the lock and local manifests.
fn collect_providers(
    dependency_set: &DependencySet,
    lock: &DependencyLock,
) -> Result<BTreeMap<String, (String, u64)>, DependencySetError> {
    let mut providers: BTreeMap<String, (String, u64)> = BTreeMap::new();

    for component in &lock.components {
        for (contract, &version) in &component.provides {
            if let Some((existing, _)) = providers.get(contract) {
                if existing != &component.name {
                    return Err(DependencySetError::DuplicateProvider {
                        contract: contract.clone(),
                        first: existing.clone(),
                        second: component.name.clone(),
                    });
                }
            }
            providers.insert(contract.clone(), (component.name.clone(), version));
        }
    }

    for manifest in dependency_set.manifests().values() {
        for (contract, &version) in &manifest.provides {
            if let Some((existing, _)) = providers.get(contract) {
                if existing != &manifest.name {
                    return Err(DependencySetError::DuplicateProvider {
                        contract: contract.clone(),
                        first: existing.clone(),
                        second: manifest.name.clone(),
                    });
                }
            }
            providers.insert(contract.clone(), (manifest.name.clone(), version));
        }
    }

    Ok(providers)
}

/// Collects every contract requirement from both manifests and locked components.
pub(super) fn collect_requirements(
    dependency_set: &DependencySet,
    lock: &DependencyLock,
) -> BTreeMap<String, Vec<ContractRequirer>> {
    let mut requirements: BTreeMap<String, Vec<ContractRequirer>> = BTreeMap::new();

    // Local manifests are authoritative for components they own.
    for manifest in dependency_set.manifests().values() {
        for (contract, range) in &manifest.requires {
            requirements
                .entry(contract.clone())
                .or_default()
                .push(ContractRequirer {
                    component: manifest.name.clone(),
                    range: range.clone(),
                });
        }
    }

    for component in &lock.components {
        if dependency_set.manifests().contains_key(&component.name) {
            continue;
        }
        for (contract, range) in &component.requires {
            requirements
                .entry(contract.clone())
                .or_default()
                .push(ContractRequirer {
                    component: component.name.clone(),
                    range: range.clone(),
                });
        }
    }

    for list in requirements.values_mut() {
        list.sort_by(|a, b| a.component.cmp(&b.component));
    }

    requirements
}

/// Intersects every requirement on a contract and returns the combined range and summary string.
pub(super) fn intersect_requirers(
    contract: &str,
    requirers: &[ContractRequirer],
) -> Result<(ContractRangeSet, String), DependencySetError> {
    let Some((first, rest)) = requirers.split_first() else {
        return Err(DependencySetError::InvalidContractRange {
            contract: contract.to_owned(),
            reason: "cannot intersect an empty requirer list".to_owned(),
        });
    };
    let mut combined = first.range.to_range_set(contract)?;
    for req in rest {
        let next = req.range.to_range_set(contract)?;
        combined = combined.intersection(&next);
    }
    let summary = requirers
        .iter()
        .map(|r| format!("{} requires {}", r.component, r.range))
        .collect::<Vec<_>>()
        .join("; ");
    Ok((combined, summary))
}
