//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Renders standard Python metadata into BSMR's private Starlark bridge.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fmt::Write;

use bsmr_core::cells::paths::CellRelativePathBuf;

use super::Manifest;
use super::NativePythonBuildError;
use super::PythonRootFiles;
use super::PythonTestLock;
use super::manifest_uses_vcs;
use super::normalize_project_name;
use super::project_files;
use super::validate_test_command;
use super::workspace_path;
use crate::package_listing::listing::PackageListing;

mod environment;

use environment::render_config_settings;
use environment::render_environment;
use environment::render_package_build_variables;
use environment::render_package_config_settings;
use environment::render_workspace_environment;

mod target {
    pub(super) const ANALYSIS_SOURCES: &str = "__bsmr_python_analysis_sources";
    pub(super) const BUILD_ENVIRONMENT: &str = "__bsmr_python_build_environment";
    pub(super) const ENVIRONMENT: &str = "__bsmr_python_environment";
    pub(super) const SOURCES: &str = "__bsmr_python_sources";
    pub(super) const VCS: &str = "__bsmr_python_vcs";
    pub(super) const WORKSPACE_ENVIRONMENT: &str = "__bsmr_python_workspace_environment";
}

struct PackageRender<'a> {
    config_files: &'a [String],
    config_settings: &'a BTreeMap<String, super::BuildConfigSetting>,
    package_config_settings: &'a BTreeMap<String, BTreeMap<String, super::BuildConfigSetting>>,
    package_build_variables: &'a BTreeMap<String, BTreeMap<String, String>>,
    package_root: &'a CellRelativePathBuf,
    listing: &'a PackageListing,
    target_name: &'a str,
    uses_vcs: bool,
    installable: bool,
    members: &'a [super::PythonWorkspaceMember],
    test_locks: &'a [PythonTestLock],
    scripts: &'a BTreeMap<String, String>,
    test_command: Option<&'a [String]>,
}

impl PackageRender<'_> {
    /// Returns the workspace-relative source root consumed by Python tools.
    fn root(&self) -> &str {
        if self.package_root.is_empty() {
            "."
        } else {
            self.package_root.as_str()
        }
    }
}

/// Renders the private Starlark bridge for one standard Python project.
#[must_use = "the generated build file must be evaluated by the interpreter"]
pub fn render_python_build_file(
    package_root: CellRelativePathBuf,
    manifest: &str,
    listing: &PackageListing,
    root_files: &PythonRootFiles,
) -> Result<String, NativePythonBuildError> {
    let manifest =
        toml::from_str::<Manifest>(manifest).map_err(NativePythonBuildError::InvalidManifest)?;
    let uses_vcs = manifest_uses_vcs(&manifest);
    let Some(project) = manifest.project else {
        return render_workspace_root(&package_root, root_files, &manifest);
    };
    if project.requires_python.is_none() {
        return Err(NativePythonBuildError::MissingProjectMetadata);
    }
    let target_name = normalize_project_name(&project.name);
    let installable = manifest.tool.uv.package != Some(false);
    validate_test_command(manifest.tool.bsmr.python.test_command.as_deref())?;
    validate_target_names(&package_root, &target_name, &project.scripts, root_files)?;

    let mut output = String::new();
    if package_root.is_empty() {
        render_environment(
            &mut output,
            root_files,
            installable.then_some(target_name.as_str()),
            &manifest.tool.uv.config_settings,
            &manifest.tool.uv.config_settings_package,
            &manifest.tool.uv.extra_build_variables,
        )?;
        render_root_config_files(&mut output, root_files)?;
    }
    render_package(
        &mut output,
        &PackageRender {
            config_files: &root_files.config_files,
            config_settings: &manifest.tool.uv.config_settings,
            package_config_settings: &manifest.tool.uv.config_settings_package,
            package_build_variables: &manifest.tool.uv.extra_build_variables,
            package_root: &package_root,
            listing,
            target_name: &target_name,
            uses_vcs,
            installable,
            members: &root_files.members,
            test_locks: &root_files.test_locks,
            scripts: &project.scripts,
            test_command: manifest.tool.bsmr.python.test_command.as_deref(),
        },
    )?;
    Ok(output)
}

/// Renders shared Python infrastructure from a non-installable workspace root.
fn render_workspace_root(
    package_root: &CellRelativePathBuf,
    root_files: &PythonRootFiles,
    manifest: &Manifest,
) -> Result<String, NativePythonBuildError> {
    if !package_root.is_empty() {
        return Err(NativePythonBuildError::MissingProjectMetadata);
    }
    let mut output = String::new();
    render_environment(
        &mut output,
        root_files,
        None,
        &manifest.tool.uv.config_settings,
        &manifest.tool.uv.config_settings_package,
        &manifest.tool.uv.extra_build_variables,
    )?;
    render_root_config_files(&mut output, root_files)?;
    Ok(output)
}

/// Exposes only standard ancestor configuration files to nested native packages.
fn render_root_config_files(
    output: &mut String,
    root_files: &PythonRootFiles,
) -> Result<(), NativePythonBuildError> {
    for file in &root_files.config_files {
        let target = root_config_target(file);
        writeln!(
            output,
            "export_file(\n    name = {target:?},\n    src = {file:?},\n    visibility = [\"PUBLIC\"],\n)\n"
        )
        .map_err(NativePythonBuildError::Render)?;
    }
    Ok(())
}

/// Returns the private target carrying one inherited root configuration file.
fn root_config_target(file: &str) -> String {
    format!("__bsmr_python_config_{}", file.replace('.', "_"))
}

/// Rejects labels that would shadow BSMR's generated target namespace.
fn validate_target_names(
    package_root: &CellRelativePathBuf,
    target_name: &str,
    scripts: &BTreeMap<String, String>,
    root_files: &PythonRootFiles,
) -> Result<(), NativePythonBuildError> {
    if is_reserved_target(target_name, &root_files.test_locks) {
        return Err(NativePythonBuildError::ReservedTargetName(
            package_root.to_owned(),
            target_name.to_owned(),
        ));
    }
    let mut script_targets = BTreeSet::new();
    for name in scripts.keys() {
        let name = if name == target_name { "run" } else { name };
        if is_reserved_target(name, &root_files.test_locks)
            || !script_targets.insert(name.to_owned())
        {
            return Err(NativePythonBuildError::ReservedTargetName(
                package_root.to_owned(),
                name.to_owned(),
            ));
        }
    }
    Ok(())
}

/// Returns whether one user-facing label belongs to BSMR's generated graph.
fn is_reserved_target(name: &str, test_locks: &[PythonTestLock]) -> bool {
    name.starts_with("__bsmr_")
        || matches!(name, "lint" | "typecheck")
        || test_locks
            .iter()
            .any(|lock| name == lock.environment || name == lock.target)
}

/// Emits source, wheel, Ruff, ty, test, and entry-point targets for one project.
fn render_package(
    output: &mut String,
    package: &PackageRender<'_>,
) -> Result<(), NativePythonBuildError> {
    writeln!(output, "load(\"@prelude//python_native:defs.bzl\", \"python_entry_point\", \"python_sources\", \"python_test\", \"python_wheel\", \"python_wheel_environment\", \"ruff_check\", \"ty_check\")\n")
        .map_err(NativePythonBuildError::Render)?;
    if !package.package_root.is_empty() && package.has_workspace_environment() {
        render_workspace_environment(
            output,
            package.members,
            package.installable.then_some(package.target_name),
            false,
        )?;
    }
    render_sources(output, package, target::SOURCES, false)?;
    render_sources(output, package, target::ANALYSIS_SOURCES, true)?;
    render_quality_targets(output, package)?;
    render_test_targets(output, package)?;
    render_entry_points(output, package)
}

impl PackageRender<'_> {
    /// Returns whether this package needs a first-party wheel layer.
    fn has_workspace_environment(&self) -> bool {
        !self.members.is_empty()
    }
}

/// Emits wheel construction plus the opinionated Ruff and ty analysis targets.
fn render_quality_targets(
    output: &mut String,
    package: &PackageRender<'_>,
) -> Result<(), NativePythonBuildError> {
    for (rule, name, sources, tool, needs_environment) in [
        (
            "python_wheel",
            package.target_name,
            target::SOURCES,
            None,
            true,
        ),
        (
            "ruff_check",
            "lint",
            target::SOURCES,
            Some("ruff = \"root//:__bsmr_ruff_distribution\""),
            false,
        ),
        (
            "ty_check",
            "typecheck",
            target::ANALYSIS_SOURCES,
            Some("ty = \"root//:__bsmr_ty_distribution\""),
            true,
        ),
    ] {
        if rule == "python_wheel" && !package.installable {
            continue;
        }
        render_quality_target(
            output,
            package,
            QualityTarget {
                rule,
                name,
                sources,
                tool,
                needs_environment,
            },
        )?;
    }
    Ok(())
}

struct QualityTarget<'a> {
    rule: &'a str,
    name: &'a str,
    sources: &'a str,
    tool: Option<&'a str>,
    needs_environment: bool,
}

/// Emits one configured wheel or static-analysis target.
fn render_quality_target(
    output: &mut String,
    package: &PackageRender<'_>,
    target_spec: QualityTarget<'_>,
) -> Result<(), NativePythonBuildError> {
    writeln!(output, "{}(", target_spec.rule).map_err(NativePythonBuildError::Render)?;
    writeln!(output, "    name = {:?},", target_spec.name)
        .map_err(NativePythonBuildError::Render)?;
    if target_spec.rule == "python_wheel" {
        writeln!(output, "    visibility = [\"PUBLIC\"],")
            .map_err(NativePythonBuildError::Render)?;
        writeln!(output, "    distribution = {:?},", package.target_name)
            .map_err(NativePythonBuildError::Render)?;
        render_config_settings(output, package.config_settings)?;
        render_package_config_settings(
            output,
            package.package_config_settings,
            Some(package.target_name),
        )?;
        render_package_build_variables(
            output,
            package.package_build_variables,
            Some(package.target_name),
        )?;
    }
    if target_spec.needs_environment {
        if target_spec.rule == "python_wheel" {
            writeln!(
                output,
                "    environment = \"root//:{}\",",
                target::BUILD_ENVIRONMENT
            )
            .map_err(NativePythonBuildError::Render)?;
        } else {
            let environment = package
                .test_locks
                .iter()
                .find(|lock| lock.target == "test")
                .map_or(target::ENVIRONMENT, |lock| lock.environment.as_str());
            writeln!(output, "    environments = [").map_err(NativePythonBuildError::Render)?;
            writeln!(output, "        \"root//:{environment}\",")
                .map_err(NativePythonBuildError::Render)?;
            writeln!(output, "    ],").map_err(NativePythonBuildError::Render)?;
        }
    }
    if target_spec.rule == "python_wheel" && package.uses_vcs {
        writeln!(output, "    vcs = \"root//:{}\",", target::VCS)
            .map_err(NativePythonBuildError::Render)?;
    }
    writeln!(
        output,
        "    python = \"root//:__bsmr_python_distribution\","
    )
    .map_err(NativePythonBuildError::Render)?;
    writeln!(output, "    project_root = {:?},", package.root())
        .map_err(NativePythonBuildError::Render)?;
    writeln!(output, "    sources = \":{}\",", target_spec.sources)
        .map_err(NativePythonBuildError::Render)?;
    if let Some(tool) = target_spec.tool {
        writeln!(output, "    {tool},").map_err(NativePythonBuildError::Render)?;
    }
    writeln!(output, ")\n").map_err(NativePythonBuildError::Render)
}

/// Emits every lock-selected native test profile.
fn render_test_targets(
    output: &mut String,
    package: &PackageRender<'_>,
) -> Result<(), NativePythonBuildError> {
    for lock in package.test_locks {
        writeln!(output, "python_test(").map_err(NativePythonBuildError::Render)?;
        writeln!(output, "    name = {:?},", lock.target)
            .map_err(NativePythonBuildError::Render)?;
        if let Some(command) = package.test_command {
            writeln!(output, "    test_command = {command:?},")
                .map_err(NativePythonBuildError::Render)?;
        }
        render_runtime_attributes(output, package, &lock.environment)?;
        writeln!(output, ")\n").map_err(NativePythonBuildError::Render)?;
    }
    Ok(())
}

/// Emits each standard project console script as a runnable target.
fn render_entry_points(
    output: &mut String,
    package: &PackageRender<'_>,
) -> Result<(), NativePythonBuildError> {
    for (name, entry) in package.scripts {
        let name = if name == package.target_name {
            "run"
        } else {
            name
        };
        writeln!(output, "python_entry_point(").map_err(NativePythonBuildError::Render)?;
        writeln!(output, "    name = {name:?},").map_err(NativePythonBuildError::Render)?;
        writeln!(output, "    entry = {entry:?},").map_err(NativePythonBuildError::Render)?;
        render_runtime_attributes(output, package, target::ENVIRONMENT)?;
        writeln!(output, ")\n").map_err(NativePythonBuildError::Render)?;
    }
    Ok(())
}

/// Emits the common declared runtime closure for tests and console scripts.
fn render_runtime_attributes(
    output: &mut String,
    package: &PackageRender<'_>,
    environment: &str,
) -> Result<(), NativePythonBuildError> {
    writeln!(output, "    environments = [").map_err(NativePythonBuildError::Render)?;
    render_workspace_environment_label(output, package)?;
    writeln!(output, "        \"root//:{environment}\",")
        .map_err(NativePythonBuildError::Render)?;
    writeln!(output, "    ],").map_err(NativePythonBuildError::Render)?;
    writeln!(output, "    project_root = {:?},", package.root())
        .map_err(NativePythonBuildError::Render)?;
    writeln!(
        output,
        "    python = \"root//:__bsmr_python_distribution\","
    )
    .map_err(NativePythonBuildError::Render)?;
    writeln!(output, "    sources = \":{}\",", target::SOURCES)
        .map_err(NativePythonBuildError::Render)
}

/// Emits the package-local first-party layer when the project requires one.
fn render_workspace_environment_label(
    output: &mut String,
    package: &PackageRender<'_>,
) -> Result<(), NativePythonBuildError> {
    if !package.has_workspace_environment() {
        return Ok(());
    }
    let label = if package.package_root.is_empty() {
        format!("root//:{}", target::WORKSPACE_ENVIRONMENT)
    } else {
        format!(":{}", target::WORKSPACE_ENVIRONMENT)
    };
    writeln!(output, "        {label:?},").map_err(NativePythonBuildError::Render)
}

/// Emits one source set at the exact invalidation granularity its consumers need.
fn render_sources(
    output: &mut String,
    package: &PackageRender<'_>,
    name: &str,
    analysis_only: bool,
) -> Result<(), NativePythonBuildError> {
    writeln!(
        output,
        "python_sources(\n    name = {name:?},\n    srcs = {{"
    )
    .map_err(NativePythonBuildError::Render)?;
    for file in project_files(package.listing, analysis_only) {
        let path = workspace_path(package.package_root, file);
        writeln!(output, "        {path:?}: {file:?},").map_err(NativePythonBuildError::Render)?;
    }
    if !package.package_root.is_empty() {
        for file in package.config_files {
            let source = format!("root//:{}", root_config_target(file));
            writeln!(output, "        {file:?}: {source:?},")
                .map_err(NativePythonBuildError::Render)?;
        }
    }
    writeln!(output, "    }},").map_err(NativePythonBuildError::Render)?;
    writeln!(output, "    visibility = [\"PUBLIC\"],\n)\n").map_err(NativePythonBuildError::Render)
}
