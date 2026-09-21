//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Decodes execution inputs from the versioned Cargo graph. Diagnostic metadata stays with the planner.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Deserialize;

use super::RustGraphError;
use super::unsupported;

/// One selected Cargo entrypoint, with dependency indices owned by this graph.
#[derive(Deserialize)]
pub(super) struct Graph {
    /// Version of this JSON protocol.
    pub schema_version: u32,
    /// Pinned Cargo library that resolved the graph.
    pub cargo_library: String,
    /// Actual compiler identity reported by its version probe.
    pub rustc_version: String,
    /// Captured root used to rebase path-package sources.
    pub workspace_root: PathBuf,
    /// Indices of the selected entrypoint units.
    pub roots: Vec<usize>,
    /// Configured compilation units with graph-local dependency indices.
    pub units: Vec<Unit>,
}

/// Preserve every configured unit, including equal-looking units with different edges.
#[derive(Deserialize)]
pub(super) struct Unit {
    /// Cargo package identity, including its source.
    pub package_id: String,
    /// Original package name before crate-name normalization.
    pub package_name: String,
    /// Native library link ownership declared by the package.
    pub package_links: Option<String>,
    /// Cargo package variables supplied to compilation.
    pub package_environment: BTreeMap<String, String>,
    /// Verified source ownership and acquisition metadata.
    pub source: Source,
    /// Cargo-discovered crate target.
    pub target: Target,
    /// Host or target context selected by Cargo.
    pub platform: Option<String>,
    /// Compiler operation whose dependency context Cargo must resolve.
    pub mode: Mode,
    /// Cargo profile after workspace and package overrides.
    pub profile: Profile,
    /// Activated features, already resolved by Cargo.
    pub features: Vec<String>,
    /// Feature names accepted by compiler cfg validation.
    pub declared_features: Vec<String>,
    /// Effective rustc arguments selected by project configuration.
    pub rustflags: Vec<String>,
    /// Lint arguments after workspace inheritance.
    pub package_lint_flags: Vec<String>,
    /// Configured linker, when Cargo overrides the toolchain default.
    pub linker: Option<PathBuf>,
    /// Configured dependency units and compiler-visible aliases.
    pub dependencies: Vec<Dependency>,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum Mode {
    Build,
    Test,
    Check,
    RunCustomBuild,
}

#[derive(Deserialize)]
pub(super) struct Source {
    /// Source or dependency classification preserved from Cargo.
    pub kind: SourceKind,
    /// Package root whose relative files belong to one source tree.
    pub root: PathBuf,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) enum SourceKind {
    Path,
    Git,
    Registry,
    SparseRegistry,
    LocalRegistry,
    Directory,
}

#[derive(Deserialize)]
pub(super) struct Target {
    /// Source or dependency classification preserved from Cargo.
    pub kind: Vec<String>,
    /// Requested output kinds, checked before lowering.
    pub crate_types: Vec<String>,
    /// Original Cargo target or profile name.
    pub name: String,
    /// Entrypoint inside the package source tree.
    pub src_path: PathBuf,
    /// Rust language edition supplied to the compiler.
    pub edition: String,
}

#[derive(Deserialize)]
pub(super) struct Dependency {
    /// Dependency unit index in the owning graph.
    pub index: usize,
    /// Rust identifier passed to --extern.
    pub extern_crate_name: String,
    /// Whether Cargo marks the dependency as public.
    pub public: bool,
    /// Whether the dependency is excluded from the extern prelude.
    pub noprelude: bool,
    /// Whether unused-extern checks exclude this dependency.
    pub nounused: bool,
}

/// Cargo has already applied workspace, package and host-build profile overrides.
#[derive(Deserialize)]
pub(super) struct Profile {
    /// Effective compiler optimization level.
    pub opt_level: String,
    /// Cross-crate optimization policy, currently restricted to off.
    pub lto: String,
    /// Alternate compiler backend, currently unsupported.
    pub codegen_backend: Option<String>,
    /// Number of compiler partitions.
    pub codegen_units: Option<u32>,
    /// Amount or named kind of debugging information.
    pub debuginfo: DebugInfo,
    /// Platform-specific debugging output policy.
    pub split_debuginfo: Option<String>,
    /// Whether debugging assertions are enabled.
    pub debug_assertions: bool,
    /// Whether arithmetic overflow is checked.
    pub overflow_checks: bool,
    /// Whether binaries carry runtime library search paths.
    pub rpath: bool,
    /// Effective panic strategy.
    pub panic: String,
    /// Effective symbol-stripping policy.
    pub strip: StripSetting,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub(super) enum DebugInfo {
    Level(u8),
    Named(String),
}

#[derive(Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum StripSetting {
    Deferred(Strip),
    Resolved(Strip),
}

#[derive(Deserialize)]
pub(super) enum Strip {
    None,
    Named(String),
}

impl Graph {
    /// Check the protocol and index boundary without recreating Cargo's resolver.
    pub fn parse(bytes: &[u8]) -> Result<Self, RustGraphError> {
        let graph: Self = serde_json::from_slice(bytes)?;
        if graph.schema_version != 1
            || graph.cargo_library != "0.98.0"
            || !graph
                .rustc_version
                .lines()
                .any(|line| ["release: 1.97.1", "release: 1.96.0-nightly"].contains(&line))
        {
            return Err(unsupported("planner", "configured graph protocol version"));
        }
        if graph.roots.is_empty()
            || graph.roots.iter().any(|index| *index >= graph.units.len())
            || graph.units.iter().any(|unit| {
                unit.dependencies
                    .iter()
                    .any(|dependency| dependency.index >= graph.units.len())
            })
        {
            return Err(unsupported(
                "planner",
                "configured graph root or dependency index",
            ));
        }
        Ok(graph)
    }
}
