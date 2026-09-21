//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Defines the versioned request and configured-unit graph.

use std::path::PathBuf;

use serde::Deserialize;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Request {
    /// Absolute manifest path inside the captured workspace.
    pub(crate) manifest: PathBuf,
    /// Cargo package selector for the requested entrypoint.
    pub(crate) package: String,
    /// Compiler operation whose dependency context Cargo must resolve.
    pub(crate) mode: Mode,
    /// Cargo target selection, independent of the compiler operation.
    pub(crate) target_filter: TargetFilter,
    /// Whether this request may acquire missing locked sources.
    pub(crate) source_policy: SourcePolicy,
    /// Explicit feature requests passed to Cargo.
    pub(crate) features: Vec<String>,
    /// Whether Cargo activates the selected package's defaults.
    pub(crate) default_features: bool,
    /// Whether Cargo activates every declared package feature.
    pub(crate) all_features: bool,
    /// Explicit compilation platform, or the compiler host.
    pub(crate) target: Option<String>,
    /// Cargo profile after workspace and package overrides.
    pub(crate) profile: String,
    /// Caller-owned Cargo home for this single request.
    pub(crate) cargo_home: PathBuf,
    /// Absolute compiler executable admitted by the caller.
    pub(crate) rustc: PathBuf,
    /// Owned scratch directory for Cargo probes and planning.
    pub(crate) target_directory: PathBuf,
}

/// Chooses whether this request may acquire locked sources before frozen planning.
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum SourcePolicy {
    Offline,
    AcquireLocked,
}

/// Selects package roots through Cargo independently of the requested compiler mode.
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum TargetFilter {
    Package,
    Library,
    Binary { name: String },
    IntegrationTest { name: String },
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Mode {
    Build,
    Test,
    Check,
}
