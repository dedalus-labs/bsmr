//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Defines the versioned request and configured-unit graph.

use std::collections::BTreeMap;
use std::path::PathBuf;

use cargo::core::Target;
use cargo::core::compiler::CompileKind;
use cargo::core::compiler::CompileMode;
use cargo::core::profiles::Profile;
use serde::Deserialize;
use serde::Serialize;

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
    /// Owned source cache, protected by one process lease.
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

#[derive(Serialize)]
pub(crate) struct Graph {
    /// Version of this JSON protocol.
    pub(crate) schema_version: u32,
    /// Pinned Cargo library that resolved the graph.
    pub(crate) cargo_library: &'static str,
    /// Actual compiler identity reported by its version probe.
    pub(crate) rustc_version: String,
    /// Captured root used to rebase path-package sources.
    pub(crate) workspace_root: PathBuf,
    /// Indices of the selected entrypoint units.
    pub(crate) roots: Vec<usize>,
    /// Configured compilation units with graph-local dependency indices.
    pub(crate) units: Vec<ConfiguredUnit>,
}

#[derive(Serialize)]
pub(crate) struct ConfiguredUnit {
    /// Cargo package identity, including its source.
    pub(crate) package_id: String,
    /// Original package name before crate-name normalization.
    pub(crate) package_name: String,
    /// Resolved package version.
    pub(crate) package_version: String,
    /// Native library link ownership declared by the package.
    pub(crate) package_links: Option<String>,
    /// Cargo package variables and build-script profile variables for this unit.
    pub(crate) package_environment: BTreeMap<String, String>,
    /// Verified source ownership and acquisition metadata.
    pub(crate) source: Source,
    /// Explicit compilation platform, or the compiler host.
    pub(crate) target: Target,
    /// Whether rustc supplies libtest or the target supplies its own entrypoint.
    pub(crate) harness: bool,
    /// Host or target context selected by Cargo.
    pub(crate) platform: CompileKind,
    /// Compiler operation whose dependency context Cargo must resolve.
    pub(crate) mode: CompileMode,
    /// Cargo profile after workspace and package overrides.
    pub(crate) profile: Profile,
    /// Activated features, already resolved by Cargo.
    pub(crate) features: Vec<String>,
    /// Feature names accepted by compiler cfg validation.
    pub(crate) declared_features: Vec<String>,
    /// Effective rustc arguments selected by project configuration.
    pub(crate) rustflags: Vec<String>,
    /// Effective documentation compiler arguments.
    pub(crate) rustdocflags: Vec<String>,
    /// Lint arguments after workspace inheritance.
    pub(crate) package_lint_flags: Vec<String>,
    /// Configured linker, when Cargo overrides the toolchain default.
    pub(crate) linker: Option<PathBuf>,
    /// Configured dependency units and compiler-visible aliases.
    pub(crate) dependencies: Vec<Dependency>,
}

#[derive(Clone, Serialize)]
pub(crate) struct Source {
    /// Immutable source input to materialize through native build rules.
    pub(crate) artifact: SourceArtifact,
    /// Source or dependency classification preserved from Cargo.
    pub(crate) kind: SourceKind,
    /// Cargo's source URL, without guessing a cache location.
    pub(crate) identity: String,
    /// Locked registry archive digest, when applicable.
    pub(crate) checksum: Option<String>,
    /// Verified local registry archive, when applicable.
    pub(crate) archive: Option<Archive>,
    /// Locked Git commit, when applicable.
    pub(crate) git_revision: Option<String>,
    /// Acquired package root used by Cargo.
    pub(crate) root: PathBuf,
    /// Absolute manifest path inside the captured workspace.
    pub(crate) manifest: PathBuf,
}

/// Source ownership required by native compilation, independent of Cargo's cache paths.
#[derive(Clone, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub(crate) enum SourceArtifact {
    /// Files already owned by the captured workspace.
    Workspace,
    /// A verified registry archive and its package directory.
    Archive {
        /// Registry download URL without credential requirements.
        url: String,
        /// SHA-256 recorded in Cargo.lock and verified during planning.
        sha256: String,
        /// Verified archive size in bytes.
        size: u64,
        /// Package directory inside the archive.
        prefix: String,
    },
    /// A pinned Git tree with a package directory inside it.
    Git {
        /// Repository URL used by Cargo.
        repository: String,
        /// Full commit identity from Cargo.lock.
        revision: String,
        /// Package path relative to the repository root.
        directory: PathBuf,
    },
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum SourceKind {
    Path,
    Git,
    Registry,
    SparseRegistry,
    LocalRegistry,
    Directory,
}

#[derive(Serialize)]
pub(crate) struct Dependency {
    /// Dependency unit index in this graph.
    pub(crate) index: usize,
    /// Rust identifier passed to --extern.
    pub(crate) extern_crate_name: String,
    /// Original dependency name before alias normalization.
    pub(crate) manifest_name: Option<String>,
    /// Manifest dependency kinds associated with this edge.
    pub(crate) manifest_kinds: Option<Vec<DependencyKind>>,
    /// Whether Cargo marks the dependency as public.
    pub(crate) public: bool,
    /// Whether the dependency is excluded from the extern prelude.
    pub(crate) noprelude: bool,
    /// Whether unused-extern checks exclude this dependency.
    pub(crate) nounused: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum DependencyKind {
    Normal,
    Build,
    Dev,
}

#[derive(Clone, Serialize)]
pub(crate) struct Archive {
    /// Verified archive file held by the source-cache lease.
    pub(crate) path: PathBuf,
    /// Archive length in bytes.
    pub(crate) size: u64,
}
