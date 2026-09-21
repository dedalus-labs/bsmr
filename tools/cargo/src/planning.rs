//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Reads Cargo configuration and exports its configured graph.

use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use cargo::GlobalContext;
use cargo::core::Package;
use cargo::core::Resolve;
use cargo::core::Workspace;
use cargo::core::compiler::BuildContext;
use cargo::core::compiler::Compilation;
use cargo::core::compiler::CompileKind;
use cargo::core::compiler::Unit;
use cargo::core::compiler::UnitInterner;
use cargo::core::compiler::UserIntent;
use cargo::core::compiler::unit_graph::UnitDep;
use cargo::core::dependency::DepKind;
use cargo::core::resolver::CliFeatures;
use cargo::ops::CompileFilter;
use cargo::ops::CompileOptions;
use cargo::ops::FilterRule;
use cargo::ops::LibRule;
use cargo::ops::Packages;
use cargo::ops::{self};
use cargo::util::context::ConfigRelativePath;
use cargo_util_terminal::Shell;

use crate::types::ConfiguredUnit;
use crate::types::Dependency;
use crate::types::DependencyKind;
use crate::types::Graph;
use crate::types::Mode;
use crate::types::Request;
use crate::types::Source;
use crate::types::SourcePolicy;
use crate::types::TargetFilter;

/// Cargo resolves locked dependencies and probes rustc without compiling source.
pub(crate) fn plan(request: Request) -> Result<Graph> {
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
    let _git_config = crate::acquisition::isolate(&request, &gctx)?;
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
    let resolve = ops::load_pkg_lockfile(&workspace)?.context("Cargo.lock has no resolution")?;
    let options = options(&request, &gctx)?;
    let interner = UnitInterner::new();
    let context = ops::create_bcx(&workspace, &options, &interner, None)?;
    ensure!(
        ["1.97.1", "1.96.0-nightly"]
            .contains(&context.target_data.rustc.version.to_string().as_str()),
        "unsupported planner compiler: {}",
        context.target_data.rustc.version
    );
    let graph = export(&context, &resolve)?;
    ensure!(
        std::fs::read(lock_path)? == lock,
        "Cargo.lock changed during planning"
    );
    Ok(graph)
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

/// Keep each Cargo unit distinct, including units with equal visible settings.
fn export(context: &BuildContext<'_, '_>, resolve: &Resolve) -> Result<Graph> {
    for unit in context.unit_graph.keys() {
        validate_unit(unit)?;
    }
    ensure!(
        context.extra_compiler_args.values().all(Vec::is_empty),
        "unsupported extra compiler arguments"
    );
    let mut units: Vec<&Unit> = context.unit_graph.keys().collect();
    let compilation = Compilation::new(context)?;
    let mut sources = BTreeMap::new();
    for unit in &units {
        if let std::collections::btree_map::Entry::Vacant(entry) =
            sources.entry(unit.pkg.package_id())
        {
            entry.insert(crate::source::export(unit, resolve, context.gctx)?);
        }
    }
    units.sort_unstable();
    let indices: BTreeMap<_, _> = units
        .iter()
        .enumerate()
        .map(|(i, unit)| (*unit, i))
        .collect();
    Ok(Graph {
        schema_version: 1,
        cargo_library: crate::CARGO_LIBRARY,
        rustc_version: context.target_data.rustc.verbose_version.clone(),
        workspace_root: context.ws.root().to_owned(),
        roots: context.roots.iter().map(|unit| indices[unit]).collect(),
        units: units
            .iter()
            .map(|unit| {
                configured_unit(
                    unit,
                    sources[&unit.pkg.package_id()].clone(),
                    compilation.target_linker(unit.kind).map(Path::to_owned),
                    context.unit_graph[*unit]
                        .iter()
                        .map(|dep| dependency(dep, &indices))
                        .collect(),
                )
            })
            .collect(),
    })
}

/// Exports Cargo's package metadata and published package identity environment.
fn package_environment(package: &Package) -> BTreeMap<String, String> {
    let version = package.version();
    let mut environment: BTreeMap<_, _> = package
        .manifest()
        .metadata()
        .env_vars()
        .map(|(key, value)| (key.to_owned(), value.into_owned()))
        .collect();
    environment.extend(
        [
            ("CARGO_PKG_NAME", package.name().to_string()),
            ("CARGO_PKG_VERSION", version.to_string()),
            ("CARGO_PKG_VERSION_MAJOR", version.major.to_string()),
            ("CARGO_PKG_VERSION_MINOR", version.minor.to_string()),
            ("CARGO_PKG_VERSION_PATCH", version.patch.to_string()),
            ("CARGO_PKG_VERSION_PRE", version.pre.to_string()),
        ]
        .into_iter()
        .map(|(key, value)| (key.to_owned(), value)),
    );
    environment
}

/// Reject Cargo units whose execution requirements are outside the native contract.
fn validate_unit(unit: &Unit) -> Result<()> {
    crate::flags::validate(&unit.rustflags, crate::flags::Phase::Compile)?;
    ensure!(
        unit.target.harness() || !unit.mode.is_any_test(),
        "unsupported custom test harness: {}",
        unit.pkg
    );
    ensure!(
        unit.links_overrides.is_empty(),
        "unsupported links override: {}",
        unit.pkg
    );
    ensure!(
        !unit.artifact.is_true() && unit.artifact_target_for_features.is_none(),
        "unsupported artifact unit: {}",
        unit.pkg
    );
    ensure!(
        !unit.is_std && !unit.skip_non_compile_time_dep,
        "unsupported std or skipped unit: {}",
        unit.pkg
    );
    Ok(())
}

/// Copy resolved compiler settings without deriving new dependency or feature policy.
fn configured_unit(
    unit: &Unit,
    source: Source,
    linker: Option<PathBuf>,
    dependencies: Vec<Dependency>,
) -> ConfiguredUnit {
    ConfiguredUnit {
        package_id: unit.pkg.package_id().to_spec().to_string(),
        package_name: unit.pkg.name().to_string(),
        package_version: unit.pkg.version().to_string(),
        package_links: unit.pkg.manifest().links().map(str::to_owned),
        package_environment: package_environment(&unit.pkg),
        source,
        target: unit.target.clone(),
        platform: unit.kind,
        mode: unit.mode,
        profile: unit.profile.clone(),
        features: unit.features.iter().map(ToString::to_string).collect(),
        declared_features: unit
            .pkg
            .summary()
            .features()
            .keys()
            .map(ToString::to_string)
            .collect(),
        rustflags: unit.rustflags.to_vec(),
        rustdocflags: unit.rustdocflags.to_vec(),
        package_lint_flags: unit.pkg.manifest().lint_rustflags().to_vec(),
        linker,
        dependencies,
    }
}

/// Preserve one resolved extern alias and its original manifest classification.
fn dependency(dep: &UnitDep, indices: &BTreeMap<&Unit, usize>) -> Dependency {
    Dependency {
        index: indices[&dep.unit],
        extern_crate_name: dep.extern_crate_name.to_string(),
        manifest_name: dep.dep_name.map(|name| name.to_string()),
        manifest_kinds: dep.manifest_deps.0.as_ref().map(|deps| {
            deps.iter()
                .map(|dep| dep.kind())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .map(|kind| match kind {
                    DepKind::Normal => DependencyKind::Normal,
                    DepKind::Build => DependencyKind::Build,
                    DepKind::Development => DependencyKind::Dev,
                })
                .collect()
        }),
        public: dep.public,
        noprelude: dep.noprelude,
        nounused: dep.nounused,
    }
}
