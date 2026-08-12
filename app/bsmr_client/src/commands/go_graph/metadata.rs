//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Deserializes the package-record stream emitted by `go list -json -test`.

//! Models the package-record stream emitted by `go list -json -test`.
//!
//! The wire model deliberately retains unsupported source classes so graph lowering
//! can reject them explicitly. It also retains Go's synthetic-test markers long
//! enough to distinguish real packages from variants produced only for `go test`.

use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;

use crate::commands::go_graph_error::GoGraphError;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub(super) struct ListedPackage {
    /// Establishes whether package sources are inside the synchronization root.
    pub(super) dir: PathBuf,
    /// Keys packages and resolves import strings to dependency nodes.
    pub(super) import_path: String,
    /// Distinguishes executable `main` packages from libraries.
    pub(super) name: String,
    /// Defines direct production dependency edges.
    #[serde(default)]
    pub(super) imports: Vec<String>,
    /// Defines imports added by in-package tests.
    #[serde(default)]
    pub(super) test_imports: Vec<String>,
    /// Defines imports added by external `package foo_test` tests.
    #[serde(default)]
    pub(super) x_test_imports: Vec<String>,
    /// Lists pure-Go production sources selected for this configuration.
    #[serde(default)]
    pub(super) go_files: Vec<String>,
    /// Lists Go production sources that invoke cgo.
    #[serde(default)]
    pub(super) cgo_files: Vec<String>,
    /// Lists C sources compiled for cgo packages.
    #[serde(default)]
    pub(super) c_files: Vec<String>,
    /// Lists C++ sources compiled for cgo packages.
    #[serde(default)]
    pub(super) cxx_files: Vec<String>,
    /// Retains unsupported Objective-C sources for fail-closed validation.
    #[serde(default)]
    pub(super) m_files: Vec<String>,
    /// Retains unsupported Fortran sources for fail-closed validation.
    #[serde(default)]
    pub(super) f_files: Vec<String>,
    /// Retains unsupported SWIG sources for fail-closed validation.
    #[serde(default)]
    pub(super) swig_files: Vec<String>,
    /// Retains unsupported C++ SWIG sources for fail-closed validation.
    #[serde(default)]
    pub(super) swig_cxx_files: Vec<String>,
    /// Lists headers addressable by package-local cgo includes.
    #[serde(default)]
    pub(super) h_files: Vec<String>,
    /// Lists assembly sources selected for this configuration.
    #[serde(default)]
    pub(super) s_files: Vec<String>,
    /// Lists precompiled system-object inputs selected by Go.
    #[serde(default)]
    pub(super) syso_files: Vec<String>,
    /// Lists tests compiled in the same package as production sources.
    #[serde(default)]
    pub(super) test_go_files: Vec<String>,
    /// Lists tests compiled as the separate external test package.
    #[serde(default)]
    pub(super) x_test_go_files: Vec<String>,
    /// Lists files matched by production `go:embed` directives.
    #[serde(default)]
    pub(super) embed_files: Vec<String>,
    /// Lists files matched by internal-test `go:embed` directives.
    #[serde(default)]
    pub(super) test_embed_files: Vec<String>,
    /// Lists files matched by external-test `go:embed` directives.
    #[serde(default)]
    pub(super) x_test_embed_files: Vec<String>,
    /// Excludes SDK standard-library packages from repository-local manifests.
    #[serde(default)]
    pub(super) standard: bool,
    /// Marks package variants synthesized while building another package's tests.
    #[serde(default)]
    pub(super) for_test: String,
    /// Prevents transitive-only packages from acquiring unintended test targets.
    #[serde(default)]
    pub(super) dep_only: bool,
    /// Preserves SDK load failures until they can become package-specific errors.
    error: Option<ListedPackageError>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ListedPackageError {
    /// Contains the diagnostic emitted by the selected SDK.
    err: String,
}

/// Deserializes the stream emitted by `go list -json` without splitting objects first.
pub(super) fn deserialize_packages(bytes: &[u8]) -> Result<Vec<ListedPackage>, GoGraphError> {
    serde_json::Deserializer::from_slice(bytes)
        .into_iter::<ListedPackage>()
        .map(|result| result.map_err(|error| GoGraphError::InvalidJson(error.to_string())))
        .collect()
}

/// Rejects SDK package errors before graph normalization can hide their context.
pub(super) fn reject_package_errors(packages: &[ListedPackage]) -> Result<(), GoGraphError> {
    for package in packages {
        if let Some(error) = &package.error {
            return Err(GoGraphError::PackageLoad {
                package: package.import_path.clone(),
                message: error.err.clone(),
            });
        }
    }
    Ok(())
}

/// Removes synthetic test variants while preserving one object per import path.
pub(super) fn base_packages(
    packages: Vec<ListedPackage>,
) -> Result<BTreeMap<String, ListedPackage>, GoGraphError> {
    let mut base = BTreeMap::new();
    for package in packages
        .into_iter()
        .filter(|package| package.for_test.is_empty() && !is_synthetic_test(package))
    {
        let import_path = package.import_path.clone();
        if base.insert(import_path.clone(), package).is_some() {
            return Err(GoGraphError::DuplicatePackage(import_path));
        }
    }
    Ok(base)
}

/// Recognizes the generated `go test` main package whose source lives in GOCACHE.
fn is_synthetic_test(package: &ListedPackage) -> bool {
    package.name == "main"
        && package.import_path.ends_with(".test")
        && package
            .go_files
            .iter()
            .any(|file| Path::new(file).is_absolute())
}
