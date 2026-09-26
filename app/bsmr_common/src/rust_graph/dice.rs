//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Infers private Rust targets from a frozen snapshot of tracked Cargo inputs.

use std::collections::BTreeMap;
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use bsmr_core::cells::cell_path::CellPath;
use bsmr_core::cells::name::CellName;
use bsmr_core::cells::paths::CellRelativePathBuf;
use bsmr_core::package::PackageLabel;
use bsmr_core::package::package_relative_path::PackageRelativePath;
use dice::DiceComputations;
use dice::Key;
use dice::OkPagableValueSerialize;
use dice::ValueSerialize;
use dice_futures::cancellation::CancellationContext;
use pagable::Pagable;
use pagable::pagable_typetag;

use super::catalog;
use super::entry::Entry;
use super::entry::Mode;
use super::invocation::Invocation;
use super::planner::Planner;
use super::selection::Selection;
use super::snapshot;
use super::toolchain::RustToolchain;
use super::unsupported;
use crate::file_ops::dice::DiceFileComputations;
use crate::file_ops::error::FileReadErrorContext;
use crate::package_listing::dice::DicePackageListingResolver;

#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    PartialEq,
    allocative::Allocative,
    derive_more::Display,
    Pagable
)]
#[display("RustGraphKey({})", _0)]
#[pagable_typetag(dice::DiceKeyDyn)]
/// Cache identity for the cell whose Cargo workspace is being inferred.
struct RustGraphKey(CellName);

#[async_trait]
impl Key for RustGraphKey {
    type Value = bsmr_error::Result<Arc<BTreeMap<String, String>>>;

    /// Recompute only when tracked resolver inputs or target names change.
    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let snapshot = tempfile::tempdir()?;
        let root = snapshot.path().canonicalize()?;
        self.capture(ctx, &root).await?;
        let source = std::fs::read_to_string(root.join("rust-toolchain.toml")).map_err(|_| {
            unsupported(
                "workspace",
                "missing rust-toolchain.toml with an exact channel",
            )
        })?;
        let toolchain = RustToolchain::parse(&source)?;
        let metadata = resolve(&toolchain, &root).await?;
        let mut rules = catalog::render(&metadata, &root, self.0.as_str())?;
        rules
            .entry(String::new())
            .or_default()
            .push_str(toolchain.rules());
        Ok(Arc::new(rules))
    }

    /// Reuse only successful, byte-identical generated graphs.
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        matches!((x, y), (Ok(x), Ok(y)) if x == y)
    }

    /// Persist successful graph values through DICE.
    fn value_serialize() -> impl ValueSerialize<Value = Self::Value> {
        OkPagableValueSerialize::<Self::Value>::new()
    }
}

impl RustGraphKey {
    /// Track package listings and materialize the resolver's private source view.
    async fn capture(&self, ctx: &mut DiceComputations<'_>, root: &Path) -> bsmr_error::Result<()> {
        let mut pending = vec![String::new()];
        // File names determine Cargo's automatic targets, contents determine its resolver.
        while let Some(directory) = pending.pop() {
            let package =
                PackageLabel::new(self.0, &CellRelativePathBuf::try_from(directory.clone())?)?;
            let listing = DicePackageListingResolver(ctx)
                .resolve_package_listing(package)
                .await?;
            for nested in listing.subpackages_within(PackageRelativePath::empty()) {
                pending.push(if directory.is_empty() {
                    nested.to_string()
                } else {
                    format!("{directory}/{nested}")
                });
            }
            for file in listing.files().files() {
                let relative = if directory.is_empty() {
                    file.to_string()
                } else {
                    format!("{directory}/{file}")
                };
                self.stage(ctx, root, &relative).await?;
            }
        }
        Ok(())
    }

    /// Track resolver contents while representing compiler inputs by their names.
    async fn stage(
        &self,
        ctx: &mut DiceComputations<'_>,
        root: &Path,
        relative: &str,
    ) -> bsmr_error::Result<()> {
        if relative
            .split('/')
            .any(|p| ["target", "bsmr-out", ".git"].contains(&p))
        {
            return Ok(());
        }
        let configuration =
            relative.ends_with(".cargo/config") || relative.ends_with(".cargo/config.toml");
        let manifest = Path::new(relative)
            .file_name()
            .is_some_and(|f| f == "Cargo.toml");
        if !manifest
            && !configuration
            && relative != "Cargo.lock"
            && relative != "rust-toolchain.toml"
            && !snapshot::inferred(Path::new(relative))
        {
            return Ok(());
        }
        let destination = root.join(relative);
        std::fs::create_dir_all(destination.parent().expect("snapshot file has parent"))?;
        let source = if manifest
            || configuration
            || relative == "Cargo.lock"
            || relative == "rust-toolchain.toml"
        {
            DiceFileComputations::read_file(
                ctx,
                CellPath::new(self.0, CellRelativePathBuf::try_from(relative.to_owned())?).as_ref(),
            )
            .await
            .without_package_context_information()?
        } else {
            String::new()
        };
        std::fs::write(destination, source)?;
        Ok(())
    }
}

/// Resolve offline without ambient configuration or changes to the captured lockfile.
async fn resolve(toolchain: &RustToolchain, root: &Path) -> bsmr_error::Result<Vec<u8>> {
    let lock = std::fs::read(root.join("Cargo.lock"))?;
    for ancestor in root.ancestors().skip(1) {
        for config in [".cargo/config", ".cargo/config.toml"] {
            if ancestor.join(config).exists() {
                return Err(unsupported("resolver", "inherited Cargo configuration").into());
            }
        }
    }
    let cargo_home = tempfile::tempdir()?;
    let output = tokio::time::timeout(
        Duration::from_secs(30),
        tokio::process::Command::new(toolchain.cargo())
            .args(["metadata", "--format-version=1", "--frozen", "--no-deps"])
            .current_dir(root)
            .env_clear()
            .env("CARGO_HOME", cargo_home.path())
            .env(
                "PATH",
                toolchain.cargo().parent().expect("Cargo has parent"),
            )
            .env("RUSTC", toolchain.rustc())
            .kill_on_drop(true)
            .output(),
    )
    .await
    .map_err(|_| unsupported("resolver", "Cargo metadata exceeded 30 seconds"))??;
    if !output.status.success() {
        return Err(unsupported("resolver", &String::from_utf8_lossy(&output.stderr)).into());
    }
    if std::fs::read(root.join("Cargo.lock"))? != lock {
        return Err(unsupported("resolver", "Cargo.lock changed during frozen resolution").into());
    }
    Ok(output.stdout)
}

/// Workspace members expose entrypoints. Other Cargo packages expose only source inputs.
pub async fn build_file(
    ctx: &mut DiceComputations<'_>,
    package: PackageLabel,
) -> bsmr_error::Result<String> {
    let rules = ctx.compute(&RustGraphKey(package.cell_name())).await??;
    match rules.get(package.as_cell_path().path().as_str()) {
        Some(member) => Ok(member.clone()),
        None => Ok(catalog::sources().to_owned()),
    }
}

/// A selected build/test graph has its own invalidation boundary.
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    PartialEq,
    allocative::Allocative,
    derive_more::Display,
    Pagable
)]
#[display("RustPlanKey({:?})", _0)]
#[pagable_typetag(dice::DiceKeyDyn)]
struct RustPlanKey(Vec<Entry>);

#[async_trait]
impl Key for RustPlanKey {
    type Value = bsmr_error::Result<Arc<String>>;

    /// Capture resolver inputs through DICE before asking Cargo for configured units.
    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let snapshot = tempfile::tempdir()?;
        let root = snapshot.path().canonicalize()?;
        let first = &self.0[0];
        RustGraphKey(first.package.cell_name())
            .capture(ctx, &root)
            .await?;
        let toolchain =
            RustToolchain::parse(&std::fs::read_to_string(root.join("rust-toolchain.toml"))?)?;
        let metadata = resolve(&toolchain, &root).await?;
        let packages = self
            .0
            .iter()
            .map(|entry| catalog::package_name(&metadata, &root, entry))
            .collect::<Result<Vec<_>, _>>()?;
        let cell = first.package.cell_name();
        let selection = Selection::read(ctx, cell, first.mode).await?;
        let bytes = Planner::new(&root, &toolchain, selection)
            .resolve(&self.0, &packages)
            .await?;
        let platform = ctx.compute(&crate::execution::ExecutionPlatformKey).await?;
        let execution = if platform
            .iter()
            .any(|(name, value)| name == "bsmr.sandbox.backend" && value == "namespace")
            && platform.iter().any(|(name, value)| {
                name == "bsmr.sandbox.profile" && value == "declared-inputs-v2"
            }) {
            super::configured::CodeExecution::DeclaredInputs
        } else {
            super::configured::CodeExecution::CompilerOnly
        };
        Ok(Arc::new(super::configured::render(
            &bytes,
            &root,
            cell.as_str(),
            &format!("{cell}//:__bsmr_rust"),
            execution,
        )?))
    }

    /// Reuse only successful byte-identical compilation definitions.
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        matches!((x, y), (Ok(x), Ok(y)) if x == y)
    }

    /// Persist successful generated graphs through the engine's normal cache.
    fn value_serialize() -> impl ValueSerialize<Value = Self::Value> {
        OkPagableValueSerialize::<Self::Value>::new()
    }
}

/// Share one parsed invocation across every requested entry in the same workspace and mode.
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    PartialEq,
    allocative::Allocative,
    Pagable,
    derive_more::Display
)]
#[display("CargoRoots({:?})", self)]
#[pagable_typetag(dice::DiceKeyDyn)]
struct CargoRoots(CellName, Mode);

#[async_trait]
impl Key for CargoRoots {
    type Value = bsmr_error::Result<Arc<Vec<Entry>>>;

    /// Resolve command roots from tracked metadata with the native pattern parser.
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let invocation = ctx.compute(&Invocation).await?;
        let Some(invocation) = invocation.as_ref() else {
            return Ok(Arc::new(Vec::new()));
        };
        let patterns = crate::pattern::parse_from_cli::parse_patterns_with_modifiers_from_cli_args(
            ctx,
            invocation.patterns(),
            invocation.working_dir(),
        )
        .await?
        .into_iter()
        .map(|pattern| pattern.parsed_pattern)
        .collect::<Vec<_>>();
        let snapshot = tempfile::tempdir()?;
        let root = snapshot.path().canonicalize()?;
        RustGraphKey(self.0).capture(ctx, &root).await?;
        let toolchain =
            RustToolchain::parse(&std::fs::read_to_string(root.join("rust-toolchain.toml"))?)?;
        let metadata = resolve(&toolchain, &root).await?;
        Ok(Arc::new(catalog::selected(
            &metadata, &root, self.0, self.1, &patterns,
        )?))
    }

    /// Pattern spelling changes can reuse a plan when the selected entries agree.
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        matches!((x, y), (Ok(x), Ok(y)) if x == y)
    }

    /// Preserve only successful selections through DICE paging.
    fn value_serialize() -> impl ValueSerialize<Value = Self::Value> {
        OkPagableValueSerialize::<Self::Value>::new()
    }
}

/// Resolve the private package requested by a public Cargo alias.
pub async fn plan_file(
    ctx: &mut DiceComputations<'_>,
    entry: &Entry,
) -> bsmr_error::Result<String> {
    let selected = ctx
        .compute(&CargoRoots(entry.package.cell_name(), entry.mode))
        .await??;
    let entries = if selected.contains(entry) {
        selected.as_ref().clone()
    } else {
        vec![entry.clone()]
    };
    let owner = &entries[0];
    if entry != owner {
        let index = entries
            .iter()
            .position(|selected| selected == entry)
            .expect("selected entry is in its plan");
        let label = format!("{}:root_{index}", owner.package_label()?);
        return Ok(format!(
            "alias(name = \"root\", actual = {}, visibility = [\"PUBLIC\"])\n",
            serde_json::to_string(&label)?
        ));
    }
    Ok(ctx.compute(&RustPlanKey(entries)).await??.as_ref().clone())
}

/// Reject nonexistent or unsupported private packages before interpreter evaluation.
pub async fn validate_entry(
    ctx: &mut DiceComputations<'_>,
    entry: &Entry,
) -> bsmr_error::Result<()> {
    plan_file(ctx, entry).await.map(|_| ())
}
