//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Defines failures that prevent Go SDK metadata from becoming a safe build graph.

//! Fail-closed errors for native Go graph normalization.

use std::path::PathBuf;

/// Errors that make a Go graph unsafe to lower into Bessemer targets.
#[derive(Debug, bsmr_error::Error)]
#[bsmr(tag = Input)]
pub(crate) enum GoGraphError {
    #[error("invalid JSON from `go list`: {0}")]
    InvalidJson(String),
    #[error("Go package `{package}` failed to load: {message}")]
    PackageLoad { package: String, message: String },
    #[error("duplicate Go package import path `{0}`")]
    DuplicatePackage(String),
    #[error("Go package `{package:?}` is outside the synchronization root `{root:?}`")]
    PackageOutsideRoot { package: PathBuf, root: PathBuf },
    #[error("Go package directory `{0:?}` is not valid UTF-8")]
    NonUtf8Directory(PathBuf),
    #[error(
        "Go dependency `{dependency}` imported by `{package}` was not returned by `go list -deps`"
    )]
    MissingDependency { package: String, dependency: String },
    #[error(
        "Go dependency `{dependency}` imported by `{package}` is outside the repository; run `go mod vendor` before `bsmr go sync`"
    )]
    NonVendoredDependency { package: String, dependency: String },
    #[error("Go package `{package}` contains unsupported source files: {files:?}")]
    UnsupportedSources { package: String, files: Vec<String> },
    #[error("Go package `{package}` returned an unsafe {kind} path `{path}`")]
    UnsafeSourcePath {
        package: String,
        kind: &'static str,
        path: String,
    },
    #[error("internal Go graph invariant failed for package `{0}`")]
    MissingNode(String),
    #[error("Go package graph contains a cycle involving: {0}")]
    Cycle(String),
}
