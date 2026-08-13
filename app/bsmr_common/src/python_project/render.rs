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
use super::normalize_project_name;
use super::project_files;
use super::workspace_path;
use crate::package_listing::listing::PackageListing;

mod target {
    pub(super) const ANALYSIS_SOURCES: &str = "__bsmr_python_analysis_sources";
    pub(super) const BUILD_ENVIRONMENT: &str = "__bsmr_python_build_environment";
    pub(super) const ENVIRONMENT: &str = "__bsmr_python_environment";
    pub(super) const SOURCES: &str = "__bsmr_python_sources";
    pub(super) const VCS: &str = "__bsmr_python_vcs";
    pub(super) const WORKSPACE_ENVIRONMENT: &str = "__bsmr_python_workspace_environment";
}

struct PackageRender<'a> {
    package_root: &'a CellRelativePathBuf,
    listing: &'a PackageListing,
    target_name: &'a str,
    dynamic_version: bool,
    test_locks: &'a [PythonTestLock],
    scripts: &'a BTreeMap<String, String>,
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
    let project = manifest
        .project
        .filter(|project| project.requires_python.is_some())
        .ok_or(NativePythonBuildError::MissingProjectMetadata)?;
    let target_name = normalize_project_name(&project.name);
    validate_target_names(&package_root, &target_name, &project.scripts, root_files)?;

    let mut output = String::new();
    if package_root.is_empty() {
        render_environment(&mut output, root_files, &target_name)?;
    }
    render_package(
        &mut output,
        &PackageRender {
            package_root: &package_root,
            listing,
            target_name: &target_name,
            dynamic_version: project.dynamic.iter().any(|field| field == "version"),
            test_locks: &root_files.test_locks,
            scripts: &project.scripts,
        },
    )?;
    Ok(output)
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

/// Emits the root toolchain, lock environments, and first-party wheel closure.
fn render_environment(
    output: &mut String,
    root_files: &PythonRootFiles,
    root_target: &str,
) -> Result<(), NativePythonBuildError> {
    writeln!(
        output,
        "load(\"@prelude//python_native:defs.bzl\", \"python_environment\", \"python_wheel_environment\")"
    )
    .map_err(NativePythonBuildError::Render)?;
    writeln!(
        output,
        "load(\"@prelude//python_native:toolchain.bzl\", \"python_native_toolchain\")\n"
    )
    .map_err(NativePythonBuildError::Render)?;
    writeln!(output, "python_native_toolchain()\n").map_err(NativePythonBuildError::Render)?;
    render_vcs(output, root_files)?;
    render_dependency_environments(output, root_files)?;
    render_workspace_environment(output, root_files, root_target)
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
    for path in ["HEAD", "objects", "refs"]
        .into_iter()
        .chain(vcs.packed_refs.then_some("packed-refs"))
        .chain(vcs.shallow.then_some("shallow"))
    {
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
) -> Result<(), NativePythonBuildError> {
    let environments = [
        (target::ENVIRONMENT, "pylock.toml"),
        (target::BUILD_ENVIRONMENT, "pylock.build.toml"),
    ]
    .into_iter()
    .map(|(name, file)| (name.to_owned(), file.to_owned()))
    .chain(
        root_files
            .test_locks
            .iter()
            .map(|lock| (lock.environment.clone(), lock.file.clone())),
    );
    for (name, lock) in environments {
        render_dependency_environment(output, &name, &lock)?;
    }
    Ok(())
}

/// Emits one exact lock installation as a CAS directory target.
fn render_dependency_environment(
    output: &mut String,
    name: &str,
    lock: &str,
) -> Result<(), NativePythonBuildError> {
    writeln!(output, "python_environment(").map_err(NativePythonBuildError::Render)?;
    writeln!(output, "    name = {name:?},").map_err(NativePythonBuildError::Render)?;
    writeln!(output, "    lock = {lock:?},").map_err(NativePythonBuildError::Render)?;
    writeln!(output, "    python = \":__bsmr_python_distribution\",")
        .map_err(NativePythonBuildError::Render)?;
    writeln!(output, "    uv = \":__bsmr_uv_distribution\",")
        .map_err(NativePythonBuildError::Render)?;
    if name != target::BUILD_ENVIRONMENT {
        writeln!(
            output,
            "    build_environment = \":{}\",",
            target::BUILD_ENVIRONMENT
        )
        .map_err(NativePythonBuildError::Render)?;
    }
    writeln!(output, "    visibility = [\"PUBLIC\"],").map_err(NativePythonBuildError::Render)?;
    writeln!(output, ")\n").map_err(NativePythonBuildError::Render)
}

/// Emits the environment that overlays all first-party wheels.
fn render_workspace_environment(
    output: &mut String,
    root_files: &PythonRootFiles,
    root_target: &str,
) -> Result<(), NativePythonBuildError> {
    writeln!(output, "python_wheel_environment(").map_err(NativePythonBuildError::Render)?;
    writeln!(output, "    name = {:?},", target::WORKSPACE_ENVIRONMENT)
        .map_err(NativePythonBuildError::Render)?;
    writeln!(output, "    python = \":__bsmr_python_distribution\",")
        .map_err(NativePythonBuildError::Render)?;
    writeln!(output, "    uv = \":__bsmr_uv_distribution\",")
        .map_err(NativePythonBuildError::Render)?;
    writeln!(output, "    wheels = [").map_err(NativePythonBuildError::Render)?;
    writeln!(output, "        {:?},", format!(":{root_target}"))
        .map_err(NativePythonBuildError::Render)?;
    for member in &root_files.members {
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

/// Emits source, wheel, Ruff, ty, test, and entry-point targets for one project.
fn render_package(
    output: &mut String,
    package: &PackageRender<'_>,
) -> Result<(), NativePythonBuildError> {
    writeln!(output, "load(\"@prelude//python_native:defs.bzl\", \"python_entry_point\", \"python_sources\", \"python_test\", \"python_wheel\", \"ruff_check\", \"ty_check\")\n")
        .map_err(NativePythonBuildError::Render)?;
    render_sources(output, package, target::SOURCES, false)?;
    render_sources(output, package, target::ANALYSIS_SOURCES, true)?;
    render_quality_targets(output, package)?;
    render_test_targets(output, package)?;
    render_entry_points(output, package)
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
            "uv = \"root//:__bsmr_uv_distribution\"",
            true,
        ),
        (
            "ruff_check",
            "lint",
            target::ANALYSIS_SOURCES,
            "ruff = \"root//:__bsmr_ruff_distribution\"",
            false,
        ),
        (
            "ty_check",
            "typecheck",
            target::ANALYSIS_SOURCES,
            "ty = \"root//:__bsmr_ty_distribution\"",
            true,
        ),
    ] {
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
    tool: &'a str,
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
    }
    if target_spec.needs_environment {
        let environment = if target_spec.rule == "python_wheel" {
            target::BUILD_ENVIRONMENT
        } else {
            target::ENVIRONMENT
        };
        writeln!(output, "    environment = \"root//:{environment}\",")
            .map_err(NativePythonBuildError::Render)?;
    }
    if target_spec.rule == "python_wheel" && package.dynamic_version {
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
    writeln!(output, "    {},", target_spec.tool).map_err(NativePythonBuildError::Render)?;
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
    writeln!(
        output,
        "        \"root//:{}\",",
        target::WORKSPACE_ENVIRONMENT
    )
    .map_err(NativePythonBuildError::Render)?;
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
    writeln!(output, "    }},\n    visibility = [\"PUBLIC\"],\n)\n")
        .map_err(NativePythonBuildError::Render)
}
