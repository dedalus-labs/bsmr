//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Lowers official Go package metadata into deterministic Bessemer graph input.

//! Normalizes the package graph selected by the official Go SDK.
//!
//! Go owns package-selection semantics; this module owns the trust boundary after
//! `go list`. It rejects unsafe or non-vendored inputs, preserves distinct internal
//! and external test packages, and orders local nodes for stable manifest output.

mod metadata;

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::Path;
use std::path::PathBuf;

use metadata::ListedPackage;
use metadata::base_packages;
use metadata::deserialize_packages;
use metadata::reject_package_errors;

use crate::commands::go_graph_error::GoGraphError;

/// A package in the normalized, repository-local Go graph.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GoPackage {
    /// Chooses the manifest destination without exposing an absolute host path.
    relative_dir: PathBuf,
    /// Preserves Go's canonical package identity for compiler import configuration.
    import_path: String,
    /// Gives every package a predictable `:lib` or `:bin` Bessemer label.
    target_name: &'static str,
    /// Contains every production input selected by the SDK, including cgo inputs.
    sources: Vec<String>,
    /// Carries direct local or vendored edges into compile action keys.
    dependencies: Vec<String>,
    /// Adds imports used only by tests compiled inside the package under test.
    test_dependencies: Vec<String>,
    /// Contains SDK-selected `package foo` test sources.
    test_files: Vec<String>,
    /// Adds imports used by the separately compiled `package foo_test` target.
    external_test_dependencies: Vec<String>,
    /// Contains SDK-selected `package foo_test` sources.
    external_test_files: Vec<String>,
    /// Makes production `go:embed` content explicit action inputs.
    embed_files: Vec<String>,
    /// Makes internal-test `go:embed` content explicit action inputs.
    test_embed_files: Vec<String>,
    /// Makes external-test `go:embed` content explicit action inputs.
    external_test_embed_files: Vec<String>,
}

impl GoPackage {
    /// Returns the package directory relative to the synchronization root.
    pub(crate) fn relative_dir(&self) -> &Path {
        &self.relative_dir
    }

    /// Returns the canonical Go import path supplied by the SDK.
    pub(crate) fn import_path(&self) -> &str {
        &self.import_path
    }

    /// Returns the stable Bessemer target name for this package kind.
    pub(crate) fn target_name(&self) -> &str {
        self.target_name
    }

    /// Returns source files selected by the exact Go SDK.
    pub(crate) fn sources(&self) -> &[String] {
        &self.sources
    }

    /// Returns direct repository-local compilation dependencies.
    pub(crate) fn dependencies(&self) -> &[String] {
        &self.dependencies
    }

    /// Returns direct repository-local test-only dependencies.
    pub(crate) fn test_dependencies(&self) -> &[String] {
        &self.test_dependencies
    }

    /// Returns test source files selected by the exact Go SDK.
    pub(crate) fn test_files(&self) -> &[String] {
        &self.test_files
    }

    /// Returns direct dependencies of the external test package.
    pub(crate) fn external_test_dependencies(&self) -> &[String] {
        &self.external_test_dependencies
    }

    /// Returns external-package test sources selected by the exact Go SDK.
    pub(crate) fn external_test_files(&self) -> &[String] {
        &self.external_test_files
    }

    /// Returns files selected for package embed directives.
    pub(crate) fn embed_files(&self) -> &[String] {
        &self.embed_files
    }

    /// Returns files selected for test embed directives.
    pub(crate) fn test_embed_files(&self) -> &[String] {
        &self.test_embed_files
    }

    /// Returns files selected for external test embed directives.
    pub(crate) fn external_test_embed_files(&self) -> &[String] {
        &self.external_test_embed_files
    }
}

/// A validated, topologically ordered Go package graph.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct GoGraph {
    /// Stores dependencies before consumers so generated output is stable.
    packages: Vec<GoPackage>,
}

impl GoGraph {
    /// Parses concatenated `go list -deps -json -test` objects.
    pub(crate) fn from_go_list(bytes: &[u8], root: &Path) -> Result<Self, GoGraphError> {
        let listed = deserialize_packages(bytes)?;
        reject_package_errors(&listed)?;
        let base = base_packages(listed)?;
        let packages = lower_packages(&base, root)?;
        Ok(Self {
            packages: topological_order(packages)?,
        })
    }

    /// Returns packages with every dependency before its consumers.
    pub(crate) fn packages(&self) -> &[GoPackage] {
        &self.packages
    }
}

/// Lowers repository-local packages and resolves their direct dependency labels.
fn lower_packages(
    listed: &BTreeMap<String, ListedPackage>,
    root: &Path,
) -> Result<BTreeMap<String, GoPackage>, GoGraphError> {
    let mut lowered = BTreeMap::new();
    for package in listed
        .values()
        .filter(|package| !package.standard && package.dir.starts_with(root))
    {
        lowered.insert(
            package.import_path.clone(),
            lower_package(package, listed, root)?,
        );
    }
    Ok(lowered)
}

/// Converts one SDK package into a repository-relative Bessemer package.
fn lower_package(
    package: &ListedPackage,
    listed: &BTreeMap<String, ListedPackage>,
    root: &Path,
) -> Result<GoPackage, GoGraphError> {
    reject_unsupported_sources(package)?;
    let relative_dir = relative_package_directory(package, root)?;
    let sources = package_sources(package);
    validate_package_sources(package, &sources)?;
    Ok(GoPackage {
        relative_dir,
        import_path: package.import_path.clone(),
        target_name: if package.name == "main" { "bin" } else { "lib" },
        sources,
        dependencies: dependency_labels(package, &package.imports, listed, root)?,
        test_dependencies: if package.dep_only {
            Vec::new()
        } else {
            dependency_labels(package, &package.test_imports, listed, root)?
        },
        test_files: selected_files(package, &package.test_go_files),
        external_test_dependencies: if package.dep_only {
            Vec::new()
        } else {
            dependency_labels(package, &package.x_test_imports, listed, root)?
        },
        external_test_files: selected_files(package, &package.x_test_go_files),
        embed_files: package.embed_files.clone(),
        test_embed_files: selected_files(package, &package.test_embed_files),
        external_test_embed_files: selected_files(package, &package.x_test_embed_files),
    })
}

/// Rejects source classes that Bessemer's Go rules cannot compile correctly.
fn reject_unsupported_sources(package: &ListedPackage) -> Result<(), GoGraphError> {
    let unsupported = package
        .m_files
        .iter()
        .chain(&package.f_files)
        .chain(&package.swig_files)
        .chain(&package.swig_cxx_files)
        .cloned()
        .collect::<Vec<_>>();
    if !unsupported.is_empty() {
        return Err(GoGraphError::UnsupportedSources {
            package: package.import_path.clone(),
            files: unsupported,
        });
    }
    Ok(())
}

/// Converts one SDK directory to a UTF-8 repository-relative package path.
fn relative_package_directory(
    package: &ListedPackage,
    root: &Path,
) -> Result<PathBuf, GoGraphError> {
    let relative_dir =
        package
            .dir
            .strip_prefix(root)
            .map_err(|_| GoGraphError::PackageOutsideRoot {
                package: package.dir.clone(),
                root: root.to_owned(),
            })?;
    relative_dir
        .to_str()
        .ok_or_else(|| GoGraphError::NonUtf8Directory(relative_dir.to_owned()))?;
    Ok(relative_dir.to_owned())
}

/// Validates every SDK-selected file class before manifest rendering.
fn validate_package_sources(
    package: &ListedPackage,
    sources: &[String],
) -> Result<(), GoGraphError> {
    validate_source_paths(package, "source", sources)?;
    validate_source_paths(package, "test source", &package.test_go_files)?;
    validate_source_paths(package, "external test source", &package.x_test_go_files)?;
    validate_source_paths(package, "embed source", &package.embed_files)?;
    validate_source_paths(package, "test embed source", &package.test_embed_files)?;
    validate_source_paths(
        package,
        "external test embed source",
        &package.x_test_embed_files,
    )?;
    Ok(())
}

/// Omits tests for dependency-only packages that the SDK did not select for testing.
fn selected_files(package: &ListedPackage, files: &[String]) -> Vec<String> {
    if package.dep_only {
        Vec::new()
    } else {
        files.to_owned()
    }
}

/// Ensures SDK metadata cannot escape the package directory in generated manifests.
fn validate_source_paths(
    package: &ListedPackage,
    kind: &'static str,
    paths: &[String],
) -> Result<(), GoGraphError> {
    for source in paths {
        let path = Path::new(source);
        if path.is_absolute()
            || path.components().any(|component| {
                matches!(
                    component,
                    std::path::Component::ParentDir | std::path::Component::RootDir
                )
            })
        {
            return Err(GoGraphError::UnsafeSourcePath {
                package: package.import_path.clone(),
                kind,
                path: source.clone(),
            });
        }
    }
    Ok(())
}

/// Collects every source class understood by Bessemer's Go rules.
fn package_sources(package: &ListedPackage) -> Vec<String> {
    let mut sources = BTreeSet::new();
    for source in package
        .go_files
        .iter()
        .chain(&package.cgo_files)
        .chain(&package.c_files)
        .chain(&package.cxx_files)
        .chain(&package.h_files)
        .chain(&package.s_files)
        .chain(&package.syso_files)
    {
        sources.insert(source.clone());
    }
    sources.into_iter().collect()
}

/// Maps Go imports to stable labels while rejecting packages outside the repository.
fn dependency_labels(
    package: &ListedPackage,
    imports: &[String],
    listed: &BTreeMap<String, ListedPackage>,
    root: &Path,
) -> Result<Vec<String>, GoGraphError> {
    let mut labels = BTreeSet::new();
    for import in imports.iter().filter(|import| import.as_str() != "C") {
        let dependency = listed
            .get(import)
            .ok_or_else(|| GoGraphError::MissingDependency {
                package: package.import_path.clone(),
                dependency: import.clone(),
            })?;
        if dependency.standard {
            continue;
        }
        if !dependency.dir.starts_with(root) {
            return Err(GoGraphError::NonVendoredDependency {
                package: package.import_path.clone(),
                dependency: import.clone(),
            });
        }
        labels.insert(target_label(&dependency.dir, &dependency.name, root)?);
    }
    Ok(labels.into_iter().collect())
}

/// Constructs the conventional `//package:lib` or `//package:bin` label.
fn target_label(dir: &Path, name: &str, root: &Path) -> Result<String, GoGraphError> {
    let relative = dir
        .strip_prefix(root)
        .map_err(|_| GoGraphError::PackageOutsideRoot {
            package: dir.to_owned(),
            root: root.to_owned(),
        })?;
    let path = relative
        .to_str()
        .ok_or_else(|| GoGraphError::NonUtf8Directory(relative.to_owned()))?
        .replace(std::path::MAIN_SEPARATOR, "/");
    let target = if name == "main" { "bin" } else { "lib" };
    Ok(if path.is_empty() {
        format!("//:{target}")
    } else {
        format!("//{path}:{target}")
    })
}

/// Orders packages with Kahn's algorithm and fails if the SDK graph is cyclic.
fn topological_order(
    packages: BTreeMap<String, GoPackage>,
) -> Result<Vec<GoPackage>, GoGraphError> {
    let label_to_import = packages
        .iter()
        .map(|(import, package)| {
            let label = package_label(package);
            (label, import.clone())
        })
        .collect::<BTreeMap<_, _>>();
    let mut incoming = packages
        .keys()
        .map(|import| (import.clone(), 0))
        .collect::<BTreeMap<_, _>>();
    let mut consumers = BTreeMap::<String, BTreeSet<String>>::new();
    for (consumer, package) in &packages {
        for dependency in &package.dependencies {
            let Some(dependency) = label_to_import.get(dependency) else {
                continue;
            };
            *incoming
                .get_mut(consumer)
                .ok_or_else(|| GoGraphError::MissingNode(consumer.clone()))? += 1;
            consumers
                .entry(dependency.clone())
                .or_default()
                .insert(consumer.clone());
        }
    }
    let mut ready = incoming
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(import, _)| import.clone())
        .collect::<BTreeSet<_>>();
    let mut ordered = Vec::with_capacity(packages.len());
    while let Some(import) = ready.pop_first() {
        ordered.push(packages[&import].clone());
        for consumer in consumers.get(&import).into_iter().flatten() {
            let count = incoming
                .get_mut(consumer)
                .ok_or_else(|| GoGraphError::MissingNode(consumer.clone()))?;
            *count -= 1;
            if *count == 0 {
                ready.insert(consumer.clone());
            }
        }
    }
    if ordered.len() != packages.len() {
        let cycle = incoming
            .into_iter()
            .filter(|(_, count)| *count > 0)
            .map(|(import, _)| import)
            .collect::<Vec<_>>()
            .join(", ");
        return Err(GoGraphError::Cycle(cycle));
    }
    Ok(ordered)
}

/// Returns a package's own stable target label.
fn package_label(package: &GoPackage) -> String {
    let path = package
        .relative_dir
        .to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/");
    if path.is_empty() {
        format!("//:{}", package.target_name)
    } else {
        format!("//{path}:{}", package.target_name)
    }
}
