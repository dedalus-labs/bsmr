//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Reads Cargo configuration and exports its configured graph.

use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use cargo::GlobalContext;
use cargo::core::Workspace;
use cargo::core::compiler::CompileKind;
use cargo::core::compiler::UnitInterner;
use cargo::core::compiler::UserIntent;
use cargo::core::resolver::CliFeatures;
use cargo::ops::CompileFilter;
use cargo::ops::CompileOptions;
use cargo::ops::FilterRule;
use cargo::ops::LibRule;
use cargo::ops::Packages;
use cargo::ops::{self};
use cargo::util::context::ConfigRelativePath;
use cargo_util_terminal::Shell;

use crate::types::Mode;
use crate::types::Request;
use crate::types::SourcePolicy;
use crate::types::TargetFilter;

/// Cargo resolves locked dependencies and probes rustc without compiling source.
pub(crate) fn plan(request: Request) -> Result<()> {
    for (name, path) in [
        ("manifest", &request.manifest),
        ("cargo_home", &request.cargo_home),
        ("rustc", &request.rustc),
        ("target_directory", &request.target_directory),
    ] {
        ensure!(path.is_absolute(), "{name} must be absolute");
    }
    ensure!(
        std::env::var_os("RUSTC_BOOTSTRAP").is_none(),
        "RUSTC_BOOTSTRAP is forbidden"
    );
    let cwd = request
        .manifest
        .parent()
        .context("manifest has no parent")?
        .to_owned();
    let mut gctx = GlobalContext::new(Shell::new(), cwd, request.cargo_home.clone());
    // cargo-as-a-library otherwise defaults to the dev channel outside its release build.
    gctx.nightly_features_allowed = false;
    compiler_ownership(&request, &gctx)?;
    crate::flags::configure(&gctx)?;
    ensure!(
        gctx.env_config()?.is_empty(),
        "unsupported Cargo environment configuration: [env]"
    );
    let storage = serde_json::to_string(&request.target_directory)?;
    let owned_config = [
        format!("build.rustc={}", serde_json::to_string(&request.rustc)?),
        format!("build.target-dir={storage}"),
        format!("build.build-dir={storage}"),
    ];
    ensure!(
        matches!(request.source_policy, SourcePolicy::Offline),
        "source acquisition is not enabled"
    );
    let offline = true;
    gctx.configure(
        0,
        true,
        None,
        offline,
        true,
        offline,
        &Some(request.target_directory.clone()),
        &[],
        &owned_config,
    )?;
    let workspace = Workspace::new(&request.manifest, &gctx)?;
    let lock_path = workspace.lock_root().as_path_unlocked().join("Cargo.lock");
    let lock = std::fs::read(&lock_path).context("a pre-existing Cargo.lock is required")?;
    let options = options(&request, &gctx)?;
    let interner = UnitInterner::new();
    let context = ops::create_bcx(&workspace, &options, &interner, None)?;
    ensure!(
        ["1.97.1", "1.96.0-nightly"]
            .contains(&context.target_data.rustc.version.to_string().as_str()),
        "unsupported planner compiler: {}",
        context.target_data.rustc.version
    );
    ensure!(
        std::fs::read(lock_path)? == lock,
        "Cargo.lock changed during planning"
    );
    for unit in context.unit_graph.keys() {
        crate::flags::validate(&unit.rustflags, crate::flags::Phase::Compile)?;
    }
    cargo::core::compiler::unit_graph::emit_serialized_unit_graph(
        &context.roots,
        &context.unit_graph,
        &gctx,
    )
}

/// Reject alternate compiler entrypoints before Cargo runs its first probe.
fn compiler_ownership(request: &Request, gctx: &GlobalContext) -> Result<()> {
    ensure!(
        request.cargo_home != request.target_directory,
        "Cargo home and target directory must be separate"
    );
    for key in [
        "RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
        "LD_PRELOAD",
        "DYLD_INSERT_LIBRARIES",
    ] {
        ensure!(
            std::env::var_os(key).is_none_or(|v| v.is_empty()),
            "unsupported compiler environment: {key}"
        );
    }
    if let Some(rustc) = std::env::var_os("RUSTC") {
        ensure!(
            Path::new(&rustc) == request.rustc,
            "RUSTC conflicts with the declared compiler"
        );
    }
    for key in ["build.rustc-wrapper", "build.rustc-workspace-wrapper"] {
        ensure!(
            gctx.get_string(key)?.is_none_or(|v| v.val.is_empty()),
            "unsupported compiler configuration: {key}"
        );
    }
    if let Some(rustc) = gctx.get::<Option<ConfigRelativePath>>("build.rustc")? {
        ensure!(
            rustc.resolve_program(gctx) == request.rustc,
            "build.rustc conflicts with the declared compiler"
        );
    }
    for key in [
        "RUSTC",
        "RUSTC_BOOTSTRAP",
        "RUSTC_WRAPPER",
        "RUSTC_WORKSPACE_WRAPPER",
        "LD_PRELOAD",
        "DYLD_INSERT_LIBRARIES",
    ] {
        ensure!(
            !gctx.env_config()?.contains_key(key),
            "unsupported compiler configuration: env.{key}"
        );
    }
    Ok(())
}

/// Pass selections through Cargo's own command option types.
fn options(request: &Request, gctx: &GlobalContext) -> Result<CompileOptions> {
    let intent = match request.mode {
        Mode::Build => UserIntent::Build,
        Mode::Test => UserIntent::Test,
        Mode::Check => UserIntent::Check { test: false },
    };
    let mut options = CompileOptions::new(gctx, intent)?;
    options.spec = Packages::Packages(vec![request.package.clone()]);
    options.filter = match &request.target_filter {
        TargetFilter::Package => CompileFilter::Default {
            required_features_filterable: true,
        },
        TargetFilter::Library => CompileFilter::lib_only(),
        TargetFilter::Binary { name } => CompileFilter::single_bin(name.clone()),
        TargetFilter::IntegrationTest { name } => CompileFilter::new(
            LibRule::False,
            FilterRule::none(),
            FilterRule::Just(vec![name.clone()]),
            FilterRule::none(),
            FilterRule::none(),
        ),
    };
    options.cli_features = CliFeatures::from_command_line(
        &request.features,
        request.all_features,
        request.default_features,
    )?;
    options.build_config.requested_profile = request.profile.as_str().into();
    let targets: Vec<_> = request.target.iter().cloned().collect();
    options.build_config.requested_kinds = CompileKind::from_requested_targets(gctx, &targets)?;
    Ok(options)
}
