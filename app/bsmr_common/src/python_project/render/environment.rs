//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Renders Python toolchains, locked environments, and first-party wheel layers.

use std::collections::BTreeMap;
use std::fmt::Write;

use sha2::Digest;
use sha2::Sha256;

use super::super::NativePythonBuildError;
use super::super::PythonRootFiles;
use super::super::PythonWorkspaceMember;
use super::target;
use crate::python_lock::PylockArtifact;
use crate::python_lock::PylockInstallationFragment;
use crate::python_lock::PylockVcsSource;

/// Emits the root toolchain, lock environments, and first-party wheel closure.
pub(super) fn render_environment(
    output: &mut String,
    root_files: &PythonRootFiles,
    root_target: Option<&str>,
    config_settings: &BTreeMap<String, super::super::BuildConfigSetting>,
    package_config_settings: &BTreeMap<String, BTreeMap<String, super::super::BuildConfigSetting>>,
    package_build_variables: &BTreeMap<String, BTreeMap<String, String>>,
) -> Result<(), NativePythonBuildError> {
    writeln!(
        output,
        "load(\"@prelude//python_native:defs.bzl\", \"python_environment\", \"python_locked_artifact\", \"python_locked_package\", \"python_wheel_environment\")"
    )
    .map_err(NativePythonBuildError::Render)?;
    writeln!(
        output,
        "load(\"@prelude//python_native:toolchain.bzl\", \"python_native_python_platform_value\", \"python_native_toolchain\")\n"
    )
    .map_err(NativePythonBuildError::Render)?;
    writeln!(output, "python_native_toolchain()\n").map_err(NativePythonBuildError::Render)?;
    render_vcs(output, root_files)?;
    render_dependency_environments(
        output,
        root_files,
        root_target,
        config_settings,
        package_config_settings,
        package_build_variables,
    )?;
    if root_target.is_some() || !root_files.members.is_empty() {
        render_workspace_environment(output, &root_files.members, root_target, true)?;
    }
    Ok(())
}

/// Emits declared Git database inputs for dynamic project versions.
fn render_vcs(
    output: &mut String,
    root_files: &PythonRootFiles,
) -> Result<(), NativePythonBuildError> {
    let Some(vcs) = &root_files.vcs else {
        return Ok(());
    };
    writeln!(
        output,
        "load(\"@prelude//python_native:defs.bzl\", \"python_vcs\")\n"
    )
    .map_err(NativePythonBuildError::Render)?;
    writeln!(output, "python_vcs(").map_err(NativePythonBuildError::Render)?;
    writeln!(output, "    name = {:?},", target::VCS).map_err(NativePythonBuildError::Render)?;
    writeln!(output, "    srcs = {{").map_err(NativePythonBuildError::Render)?;
    for path in &vcs.files {
        writeln!(
            output,
            "        {path:?}: {git_path:?},",
            git_path = format!(".git/{path}")
        )
        .map_err(NativePythonBuildError::Render)?;
    }
    writeln!(output, "    }},\n    visibility = [\"PUBLIC\"],\n)\n")
        .map_err(NativePythonBuildError::Render)
}

/// Emits one dependency environment for each authoritative PEP 751 lock.
fn render_dependency_environments(
    output: &mut String,
    root_files: &PythonRootFiles,
    root_target: Option<&str>,
    config_settings: &BTreeMap<String, super::super::BuildConfigSetting>,
    package_config_settings: &BTreeMap<String, BTreeMap<String, super::super::BuildConfigSetting>>,
    package_build_variables: &BTreeMap<String, BTreeMap<String, String>>,
) -> Result<(), NativePythonBuildError> {
    validate_directory_sources(root_files, root_target)?;
    let mut actions = BTreeMap::new();
    index_locked_packages(
        &mut actions,
        &root_files.build_packages,
        None,
        config_settings,
        package_config_settings,
        package_build_variables,
    )?;
    index_locked_packages(
        &mut actions,
        &root_files.runtime_packages,
        Some(target::BUILD_ENVIRONMENT),
        config_settings,
        package_config_settings,
        package_build_variables,
    )?;
    for lock in &root_files.test_locks {
        index_locked_packages(
            &mut actions,
            &lock.packages,
            Some(target::BUILD_ENVIRONMENT),
            config_settings,
            package_config_settings,
            package_build_variables,
        )?;
    }
    let mut artifacts = BTreeMap::new();
    for (package, _) in actions.values() {
        if let Some(artifact) = &package.artifact {
            artifacts.insert(locked_artifact_target(artifact), artifact);
        }
        for artifact in package.platform_artifacts.values() {
            artifacts.insert(locked_artifact_target(artifact), artifact);
        }
        if let Some(source) = &package.source_artifact {
            artifacts.insert(locked_artifact_target(&source.artifact), &source.artifact);
        }
        for artifact in package.artifacts.values().flatten() {
            artifacts.insert(locked_artifact_target(artifact), artifact);
        }
    }
    for (name, artifact) in artifacts {
        render_locked_artifact(output, &name, artifact)?;
    }
    let vcs_sources = actions
        .values()
        .filter_map(|(package, _)| package.vcs_source.as_ref())
        .map(|source| (locked_vcs_target(source), source))
        .collect::<BTreeMap<_, _>>();
    for (name, source) in vcs_sources {
        render_locked_vcs(output, &name, source)?;
    }
    for (name, (package, build_environment)) in actions {
        render_locked_package(
            output,
            &name,
            package,
            build_environment,
            config_settings,
            package_config_settings,
            package_build_variables,
        )?;
    }
    render_dependency_environment(
        output,
        target::BUILD_ENVIRONMENT,
        &root_files.build_packages,
        None,
        config_settings,
        package_config_settings,
        package_build_variables,
    )?;
    render_dependency_environment(
        output,
        target::ENVIRONMENT,
        &root_files.runtime_packages,
        Some(target::BUILD_ENVIRONMENT),
        config_settings,
        package_config_settings,
        package_build_variables,
    )?;
    for lock in &root_files.test_locks {
        render_dependency_environment(
            output,
            &lock.environment,
            &lock.packages,
            Some(target::BUILD_ENVIRONMENT),
            config_settings,
            package_config_settings,
            package_build_variables,
        )?;
    }
    Ok(())
}

/// Emits one digest-verified distribution acquisition action.
fn render_locked_artifact(
    output: &mut String,
    name: &str,
    artifact: &PylockArtifact,
) -> Result<(), NativePythonBuildError> {
    writeln!(output, "python_locked_artifact(").map_err(NativePythonBuildError::Render)?;
    writeln!(output, "    name = {name:?},").map_err(NativePythonBuildError::Render)?;
    writeln!(output, "    filename = {:?},", artifact.filename)
        .map_err(NativePythonBuildError::Render)?;
    writeln!(output, "    sha256 = {:?},", artifact.sha256)
        .map_err(NativePythonBuildError::Render)?;
    writeln!(output, "    size = {},", artifact.size).map_err(NativePythonBuildError::Render)?;
    writeln!(output, "    url = {:?},", artifact.url).map_err(NativePythonBuildError::Render)?;
    writeln!(output, ")\n").map_err(NativePythonBuildError::Render)
}

/// Emits one exact Git commit as a cacheable source-tree acquisition action.
fn render_locked_vcs(
    output: &mut String,
    name: &str,
    source: &PylockVcsSource,
) -> Result<(), NativePythonBuildError> {
    writeln!(output, "git_fetch(").map_err(NativePythonBuildError::Render)?;
    writeln!(output, "    name = {name:?},").map_err(NativePythonBuildError::Render)?;
    writeln!(output, "    repo = {:?},", source.url).map_err(NativePythonBuildError::Render)?;
    writeln!(output, "    rev = {:?},", source.commit).map_err(NativePythonBuildError::Render)?;
    if source.commit.len() == 64 {
        writeln!(output, "    object_format = \"sha256\",")
            .map_err(NativePythonBuildError::Render)?;
    }
    writeln!(output, ")\n").map_err(NativePythonBuildError::Render)
}

/// Indexes each distinct package action once across every lock profile.
fn index_locked_packages<'a>(
    actions: &mut BTreeMap<String, (&'a PylockInstallationFragment, Option<&'a str>)>,
    packages: &'a [PylockInstallationFragment],
    build_environment: Option<&'a str>,
    config_settings: &BTreeMap<String, super::super::BuildConfigSetting>,
    package_config_settings: &BTreeMap<String, BTreeMap<String, super::super::BuildConfigSetting>>,
    package_build_variables: &BTreeMap<String, BTreeMap<String, String>>,
) -> Result<(), NativePythonBuildError> {
    for package in packages {
        if package.directory_source.is_some() {
            continue;
        }
        let build_environment = build_environment.filter(|_| package.acquisition.permits_source());
        let name = locked_package_target(
            package,
            build_environment,
            config_settings,
            package_config_settings,
            package_build_variables,
        )?;
        actions.entry(name).or_insert((package, build_environment));
    }
    Ok(())
}

/// Emits package actions plus one deterministic merged environment.
fn render_dependency_environment(
    output: &mut String,
    name: &str,
    packages: &[PylockInstallationFragment],
    build_environment: Option<&str>,
    config_settings: &BTreeMap<String, super::super::BuildConfigSetting>,
    package_config_settings: &BTreeMap<String, BTreeMap<String, super::super::BuildConfigSetting>>,
    package_build_variables: &BTreeMap<String, BTreeMap<String, String>>,
) -> Result<(), NativePythonBuildError> {
    writeln!(output, "python_environment(").map_err(NativePythonBuildError::Render)?;
    writeln!(output, "    name = {name:?},").map_err(NativePythonBuildError::Render)?;
    writeln!(output, "    packages = [").map_err(NativePythonBuildError::Render)?;
    for package in packages {
        if package.directory_source.is_some() {
            continue;
        }
        let name = locked_package_target(
            package,
            build_environment.filter(|_| package.acquisition.permits_source()),
            config_settings,
            package_config_settings,
            package_build_variables,
        )?;
        writeln!(output, "        {:?},", format!(":{name}"))
            .map_err(NativePythonBuildError::Render)?;
    }
    writeln!(output, "    ],").map_err(NativePythonBuildError::Render)?;
    writeln!(output, "    python = \":__bsmr_python_distribution\",")
        .map_err(NativePythonBuildError::Render)?;
    writeln!(output, "    visibility = [\"PUBLIC\"],").map_err(NativePythonBuildError::Render)?;
    writeln!(output, ")\n").map_err(NativePythonBuildError::Render)
}

/// Requires every local lock entry to reuse one existing first-party wheel action.
fn validate_directory_sources(
    root_files: &PythonRootFiles,
    root_target: Option<&str>,
) -> Result<(), NativePythonBuildError> {
    for package in &root_files.build_packages {
        if let Some(source) = &package.directory_source {
            return Err(NativePythonBuildError::LocalDirectoryBuildRequirement {
                package: package.package.clone(),
                path: display_directory_path(&source.path),
            });
        }
    }
    for package in root_files.runtime_packages.iter().chain(
        root_files
            .test_locks
            .iter()
            .flat_map(|lock| lock.packages.iter()),
    ) {
        let Some(source) = &package.directory_source else {
            continue;
        };
        let root_matches = source.path.is_empty() && root_target == Some(&package.package);
        let member_matches = root_files
            .members
            .iter()
            .any(|member| member.package == source.path && member.target == package.package);
        if !root_matches && !member_matches {
            return Err(NativePythonBuildError::UnknownLocalDirectorySource {
                package: package.package.clone(),
                path: display_directory_path(&source.path),
            });
        }
    }
    Ok(())
}

/// Renders the lock directory itself using its standard relative spelling.
fn display_directory_path(path: &str) -> String {
    if path.is_empty() {
        ".".to_owned()
    } else {
        path.to_owned()
    }
}

/// Emits one normalized package as an independently cacheable uv action.
fn render_locked_package(
    output: &mut String,
    name: &str,
    package: &PylockInstallationFragment,
    build_environment: Option<&str>,
    config_settings: &BTreeMap<String, super::super::BuildConfigSetting>,
    package_config_settings: &BTreeMap<String, BTreeMap<String, super::super::BuildConfigSetting>>,
    package_build_variables: &BTreeMap<String, BTreeMap<String, String>>,
) -> Result<(), NativePythonBuildError> {
    writeln!(output, "python_locked_package(").map_err(NativePythonBuildError::Render)?;
    writeln!(output, "    name = {:?},", name).map_err(NativePythonBuildError::Render)?;
    writeln!(output, "    lock = {:?},", package.contents)
        .map_err(NativePythonBuildError::Render)?;
    writeln!(output, "    package = {:?},", package.package)
        .map_err(NativePythonBuildError::Render)?;
    if let Some(artifact) = &package.artifact {
        writeln!(
            output,
            "    artifact = {:?},",
            format!(":{}", locked_artifact_target(artifact))
        )
        .map_err(NativePythonBuildError::Render)?;
    }
    if !package.platform_artifacts.is_empty() {
        writeln!(
            output,
            "    artifact = python_native_python_platform_value({{"
        )
        .map_err(NativePythonBuildError::Render)?;
        for (platform, artifact) in &package.platform_artifacts {
            writeln!(
                output,
                "        {platform:?}: {:?},",
                format!(":{}", locked_artifact_target(artifact))
            )
            .map_err(NativePythonBuildError::Render)?;
        }
        writeln!(output, "    }}),").map_err(NativePythonBuildError::Render)?;
    }
    if let Some(source) = &package.source_artifact {
        writeln!(
            output,
            "    source_artifact = {:?},",
            format!(":{}", locked_artifact_target(&source.artifact))
        )
        .map_err(NativePythonBuildError::Render)?;
        if let Some(subdirectory) = &source.subdirectory {
            writeln!(output, "    source_subdirectory = {subdirectory:?},")
                .map_err(NativePythonBuildError::Render)?;
        }
        writeln!(output, "    source_version = {:?},", source.version)
            .map_err(NativePythonBuildError::Render)?;
    }
    if let Some(source) = &package.vcs_source {
        writeln!(
            output,
            "    source_tree = {:?},",
            format!(":{}", locked_vcs_target(source))
        )
        .map_err(NativePythonBuildError::Render)?;
        if let Some(subdirectory) = &source.subdirectory {
            writeln!(output, "    source_subdirectory = {subdirectory:?},")
                .map_err(NativePythonBuildError::Render)?;
        }
        writeln!(output, "    source_version = {:?},", source.version)
            .map_err(NativePythonBuildError::Render)?;
    }
    if !package.artifacts.is_empty() {
        writeln!(
            output,
            "    artifacts = python_native_python_platform_value({{"
        )
        .map_err(NativePythonBuildError::Render)?;
        for (platform, artifacts) in &package.artifacts {
            writeln!(output, "        {platform:?}: [").map_err(NativePythonBuildError::Render)?;
            for artifact in artifacts {
                writeln!(
                    output,
                    "            {:?},",
                    format!(":{}", locked_artifact_target(artifact))
                )
                .map_err(NativePythonBuildError::Render)?;
            }
            writeln!(output, "        ],").map_err(NativePythonBuildError::Render)?;
        }
        writeln!(output, "    }}),").map_err(NativePythonBuildError::Render)?;
    }
    writeln!(
        output,
        "    acquisition = {:?},",
        package.acquisition.as_str()
    )
    .map_err(NativePythonBuildError::Render)?;
    if package.acquisition.permits_source() {
        render_config_settings(output, config_settings)?;
        render_package_config_settings(output, package_config_settings, Some(&package.package))?;
        render_package_build_variables(output, package_build_variables, Some(&package.package))?;
    }
    writeln!(output, "    python = \":__bsmr_python_distribution\",")
        .map_err(NativePythonBuildError::Render)?;
    writeln!(output, "    uv = \":__bsmr_uv_distribution\",")
        .map_err(NativePythonBuildError::Render)?;
    if let Some(build_environment) = build_environment {
        writeln!(output, "    build_environment = \":{build_environment}\",")
            .map_err(NativePythonBuildError::Render)?;
    }
    writeln!(output, ")\n").map_err(NativePythonBuildError::Render)
}

/// Returns the stable target identity for one downloadable distribution.
fn locked_artifact_target(artifact: &PylockArtifact) -> String {
    let mut digest = Sha256::new();
    for value in [
        artifact.filename.as_str(),
        artifact.sha256.as_str(),
        artifact.url.as_str(),
    ] {
        digest.update(value.as_bytes());
        digest.update([0]);
    }
    digest.update(artifact.size.to_le_bytes());
    format!("__bsmr_python_artifact__{}", hex::encode(digest.finalize()))
}

/// Returns the stable target identity for one immutable Git tree.
fn locked_vcs_target(source: &PylockVcsSource) -> String {
    let mut digest = Sha256::new();
    digest.update(source.url.as_bytes());
    digest.update([0]);
    digest.update(source.commit.as_bytes());
    format!("__bsmr_python_vcs__{}", hex::encode(digest.finalize()))
}

/// Returns one content-derived label for a package action and its build contract.
fn locked_package_target(
    package: &PylockInstallationFragment,
    build_environment: Option<&str>,
    config_settings: &BTreeMap<String, super::super::BuildConfigSetting>,
    package_config_settings: &BTreeMap<String, BTreeMap<String, super::super::BuildConfigSetting>>,
    package_build_variables: &BTreeMap<String, BTreeMap<String, String>>,
) -> Result<String, NativePythonBuildError> {
    let mut configuration = String::new();
    if package.acquisition.permits_source() {
        render_config_settings(&mut configuration, config_settings)?;
        render_package_config_settings(
            &mut configuration,
            package_config_settings,
            Some(&package.package),
        )?;
        render_package_build_variables(
            &mut configuration,
            package_build_variables,
            Some(&package.package),
        )?;
    }
    let mut digest = Sha256::new();
    digest.update(package.contents.as_bytes());
    digest.update([0]);
    digest.update(configuration.as_bytes());
    digest.update([0]);
    digest.update(build_environment.unwrap_or_default().as_bytes());
    Ok(format!(
        "__bsmr_python_package__{}__{}",
        package.package,
        hex::encode(digest.finalize())
    ))
}

/// Emits package-specific PEP 517 settings in canonical package, key, and value order.
pub(super) fn render_package_config_settings(
    output: &mut String,
    settings: &BTreeMap<String, BTreeMap<String, super::super::BuildConfigSetting>>,
    selected_package: Option<&str>,
) -> Result<(), NativePythonBuildError> {
    let settings = settings.iter().filter(|(package, _)| {
        selected_package
            .is_none_or(|selected| super::super::normalize_project_name(package) == selected)
    });
    let mut settings = settings.peekable();
    if settings.peek().is_none() {
        return Ok(());
    }
    writeln!(output, "    package_config_settings = [").map_err(NativePythonBuildError::Render)?;
    for (package, package_settings) in settings {
        for (name, setting) in package_settings {
            for value in setting.values() {
                writeln!(output, "        {:?},", format!("{package}:{name}={value}"))
                    .map_err(NativePythonBuildError::Render)?;
            }
        }
    }
    writeln!(output, "    ],").map_err(NativePythonBuildError::Render)
}

/// Emits package-scoped build environment variables in canonical order.
pub(super) fn render_package_build_variables(
    output: &mut String,
    variables: &BTreeMap<String, BTreeMap<String, String>>,
    selected_package: Option<&str>,
) -> Result<(), NativePythonBuildError> {
    let variables = variables.iter().filter(|(package, _)| {
        selected_package
            .is_none_or(|selected| super::super::normalize_project_name(package) == selected)
    });
    let mut variables = variables.peekable();
    if variables.peek().is_none() {
        return Ok(());
    }
    writeln!(output, "    package_build_variables = [").map_err(NativePythonBuildError::Render)?;
    for (package, package_variables) in variables {
        for (name, value) in package_variables {
            writeln!(output, "        {:?},", format!("{package}:{name}={value}"))
                .map_err(NativePythonBuildError::Render)?;
        }
    }
    writeln!(output, "    ],").map_err(NativePythonBuildError::Render)
}

/// Emits PEP 517 settings in canonical key and repetition order.
pub(super) fn render_config_settings(
    output: &mut String,
    settings: &BTreeMap<String, super::super::BuildConfigSetting>,
) -> Result<(), NativePythonBuildError> {
    if settings.is_empty() {
        return Ok(());
    }
    writeln!(output, "    config_settings = {{").map_err(NativePythonBuildError::Render)?;
    for (name, setting) in settings {
        writeln!(output, "        {name:?}: {:?},", setting.values())
            .map_err(NativePythonBuildError::Render)?;
    }
    writeln!(output, "    }},").map_err(NativePythonBuildError::Render)
}

/// Emits the environment that overlays exact first-party wheels.
pub(super) fn render_workspace_environment(
    output: &mut String,
    members: &[PythonWorkspaceMember],
    root_target: Option<&str>,
    local_tools: bool,
) -> Result<(), NativePythonBuildError> {
    writeln!(output, "python_wheel_environment(").map_err(NativePythonBuildError::Render)?;
    writeln!(output, "    name = {:?},", target::WORKSPACE_ENVIRONMENT)
        .map_err(NativePythonBuildError::Render)?;
    let tool_prefix = if local_tools { ":" } else { "root//:" };
    writeln!(
        output,
        "    python = {:?},",
        format!("{tool_prefix}__bsmr_python_distribution")
    )
    .map_err(NativePythonBuildError::Render)?;
    writeln!(
        output,
        "    uv = {:?},",
        format!("{tool_prefix}__bsmr_uv_distribution")
    )
    .map_err(NativePythonBuildError::Render)?;
    writeln!(output, "    wheels = [").map_err(NativePythonBuildError::Render)?;
    if let Some(root_target) = root_target {
        writeln!(output, "        {:?},", format!(":{root_target}"))
            .map_err(NativePythonBuildError::Render)?;
    }
    for member in members {
        writeln!(
            output,
            "        {label:?},",
            label = format!("//{}:{}", member.package, member.target)
        )
        .map_err(NativePythonBuildError::Render)?;
    }
    writeln!(output, "    ],\n    visibility = [\"PUBLIC\"],\n)\n")
        .map_err(NativePythonBuildError::Render)
}
