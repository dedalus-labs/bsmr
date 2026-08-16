//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Validates the frozen PEP 517 closure selected by standard project metadata.

use std::collections::BTreeSet;

use super::ExtraBuildDependency;
use super::Manifest;
use super::NativePythonBuildError;
use super::PythonWorkspaceMember;
use super::normalize_project_name;
use super::requirement_names;
use crate::python_lock::PylockToml;

/// Verifies that the frozen build lock covers one package-local build closure.
#[must_use = "missing build requirements must fail before any backend executes"]
pub fn validate_python_build_requirements(
    manifest: &str,
    members: &[PythonWorkspaceMember],
    runtime_lock: &PylockToml,
    build_lock: &PylockToml,
) -> Result<(), NativePythonBuildError> {
    let manifest =
        toml::from_str::<Manifest>(manifest).map_err(NativePythonBuildError::InvalidManifest)?;
    let requirements = manifest_build_requirements(&manifest)?
        .into_iter()
        .chain(
            members
                .iter()
                .flat_map(|member| member.build_requirements.iter().cloned()),
        )
        .collect::<BTreeSet<_>>();
    let available = build_lock
        .packages
        .iter()
        .map(|package| normalize_project_name(&package.name))
        .collect::<BTreeSet<_>>();
    if let Some(requirement) = requirements
        .iter()
        .find(|requirement| !available.contains(*requirement))
    {
        return Err(NativePythonBuildError::MissingBuildRequirement(
            requirement.to_owned(),
        ));
    }
    validate_runtime_matches(&manifest, runtime_lock, build_lock)
}

/// Returns backend and uv compatibility requirements in canonical name order.
pub(super) fn manifest_build_requirements(
    manifest: &Manifest,
) -> Result<Vec<String>, NativePythonBuildError> {
    let mut requirements = manifest
        .build_system
        .as_ref()
        .map(|build| build.requires.as_slice())
        .unwrap_or_default()
        .to_vec();
    requirements.extend(
        manifest
            .tool
            .uv
            .extra_build_dependencies
            .values()
            .flatten()
            .map(ExtraBuildDependency::requirement)
            .map(str::to_owned),
    );
    Ok(requirement_names(&requirements)?
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect())
}

/// Enforces uv's exact build/runtime version coupling when requested.
fn validate_runtime_matches(
    manifest: &Manifest,
    runtime_lock: &PylockToml,
    build_lock: &PylockToml,
) -> Result<(), NativePythonBuildError> {
    for dependency in manifest
        .tool
        .uv
        .extra_build_dependencies
        .values()
        .flatten()
        .filter(|dependency| dependency.match_runtime())
    {
        let package = requirement_names(&[dependency.requirement().to_owned()])?
            .pop()
            .expect("one parsed requirement");
        let runtime = locked_versions(runtime_lock, &package);
        let build = locked_versions(build_lock, &package);
        if runtime != build {
            return Err(NativePythonBuildError::BuildRequirementVersionMismatch {
                package,
                runtime,
                build,
            });
        }
    }
    Ok(())
}

/// Returns the canonical version set for one normalized package in a lock.
fn locked_versions(lock: &PylockToml, package: &str) -> Vec<String> {
    lock.packages
        .iter()
        .filter(|candidate| normalize_project_name(&candidate.name) == package)
        .filter_map(|candidate| candidate.version.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}
