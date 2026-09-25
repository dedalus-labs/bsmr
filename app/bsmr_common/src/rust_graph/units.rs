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
    /// Cargo package boundaries, including unselected workspace members.
    pub workspace_packages: Vec<PathBuf>,
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
    /// Cargo version passed literally to the build-script rule.
    pub package_version: String,
    /// Native library link ownership declared by the package.
    pub package_links: Option<String>,
    /// Cargo package variables supplied to compilation.
    pub package_environment: BTreeMap<String, String>,
    /// Verified source ownership and acquisition metadata.
    pub source: Source,
    /// Cargo-discovered crate target.
    pub target: Target,
    /// Whether this target uses rustc's test harness rather than its own main.
    pub harness: bool,
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
    /// Package root whose relative files belong to one source tree.
    pub root: PathBuf,
    /// Immutable source ownership exported by the planner.
    pub artifact: SourceArtifact,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub(super) enum SourceArtifact {
    /// A package inside the captured workspace.
    Workspace,
    /// Registry bytes verified against Cargo.lock.
    Archive {
        /// Credential-free archive endpoint.
        url: String,
        /// Expected SHA-256 before extraction.
        sha256: String,
        /// Expected archive length in bytes.
        size: u64,
        /// Source-tree directory inside the archive.
        prefix: String,
        /// Package directory relative to that source tree.
        package: String,
    },
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
    /// Link-time optimization policy selected by Cargo.
    pub lto: Lto,
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

/// Cargo distinguishes disabled optimization from optimization within one crate.
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum Lto {
    /// Disable even optimization between one crate's codegen units.
    Off,
    /// Let rustc optimize within one crate when its profile permits it.
    #[serde(rename = "false")]
    Local,
    /// Optimize the complete dependency graph together.
    #[serde(alias = "true")]
    Fat,
    /// Optimize across crates using LLVM's thin LTO mode.
    Thin,
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
        if graph.schema_version != 5
            || graph.cargo_library != "0.98.0"
            || !graph.rustc_version.lines().any(|line| {
                [
                    "release: 1.97.1",
                    "release: 1.98.0",
                    "release: 1.96.0-nightly",
                ]
                .contains(&line)
            })
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
