//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Infers private Rust targets from a frozen snapshot of tracked Cargo inputs.

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
use super::entry::Origin;
use super::entry::Requested;
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
    type Value = bsmr_error::Result<Arc<catalog::Catalog>>;

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
        let mut catalog = catalog::render(&metadata, &root, self.0.as_str())?;
        catalog
            .rules
            .entry(String::new())
            .or_default()
            .push_str(toolchain.rules());
        Ok(Arc::new(catalog))
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
    /// Preserve existing custom entrypoint paths without making absent source files exist.
    async fn entrypoints(
        &self,
        ctx: &mut DiceComputations<'_>,
        root: &Path,
        metadata: &[u8],
    ) -> bsmr_error::Result<()> {
        for source in catalog::entrypoints(metadata)? {
            let relative = source
                .strip_prefix(root)
                .map_err(|_| super::RustGraphError::Outside(source.clone()))?;
            let path = CellPath::new(
                self.0,
                CellRelativePathBuf::try_from(relative.to_string_lossy().replace('\\', "/"))?,
            );
            if source.try_exists()? {
                continue;
            }
            if DiceFileComputations::exists_matching_exact_case(ctx, path.as_ref()).await? {
                std::fs::create_dir_all(source.parent().expect("target source has a parent"))?;
                std::fs::write(source, "")?;
            }
        }
        Ok(())
    }

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
    let catalog = ctx.compute(&RustGraphKey(package.cell_name())).await??;
    match catalog.rules.get(package.as_cell_path().path().as_str()) {
        Some(member) => Ok(member.clone()),
        None => Ok(catalog::sources().to_owned()),
    }
}

/// Expand a Cargo directory using the same metadata that defines its public targets.
pub async fn directory(
    ctx: &mut DiceComputations<'_>,
    path: CellPath,
) -> bsmr_error::Result<Option<Vec<String>>> {
    use bsmr_fs::paths::forward_rel_path::ForwardRelativePath;

    use crate::package_listing::PackageBuildSource;
    use crate::package_listing::find_build_source;

    let manifest = path.join(ForwardRelativePath::unchecked_new("Cargo.toml"));
    if !DiceFileComputations::exists_matching_exact_case(ctx, manifest.as_ref()).await? {
        return Ok(None);
    }
    let listing = DiceFileComputations::read_dir(ctx, path.as_ref()).await?;
    let buildfiles = DiceFileComputations::buildfiles(ctx, path.cell()).await?;
    if find_build_source(&buildfiles, &listing.included, true)
        .is_none_or(|(_, source)| source != PackageBuildSource::Native)
    {
        return Ok(None);
    }
    let catalog = ctx.compute(&RustGraphKey(path.cell())).await??;
    let labels = catalog
        .directories
        .get(path.path().as_str())
        .ok_or_else(|| super::RustGraphError::NotWorkspaceMember(path.clone()))?;
    if labels.is_empty() {
        return Err(unsupported(&path.to_string(), "empty default target selection").into());
    }
    Ok(Some(labels.clone()))
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
struct RustPlanKey(Vec<Requested>);

/// The resolver owns both enabled roots and their compiler definitions.
#[derive(Debug, Eq, PartialEq, allocative::Allocative, Pagable)]
struct Plan {
    /// Enabled roots in the exact order returned by Cargo.
    roots: Vec<Entry>,
    /// Native definitions for those roots and their reachable dependencies.
    source: String,
}

#[async_trait]
impl Key for RustPlanKey {
    type Value = bsmr_error::Result<Arc<Plan>>;

    /// Capture resolver inputs through DICE before asking Cargo for configured units.
    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let snapshot = tempfile::tempdir()?;
        let root = snapshot.path().canonicalize()?;
        let first = &self.0[0].entry;
        RustGraphKey(first.package.cell_name())
            .capture(ctx, &root)
            .await?;
        let toolchain =
            RustToolchain::parse(&std::fs::read_to_string(root.join("rust-toolchain.toml"))?)?;
        let metadata = resolve(&toolchain, &root).await?;
        RustGraphKey(first.package.cell_name())
            .entrypoints(ctx, &root, &metadata)
            .await?;
        let packages = self
            .0
            .iter()
            .map(|entry| catalog::package_name(&metadata, &root, &entry.entry))
            .collect::<Result<Vec<_>, _>>()?;
        let cell = first.package.cell_name();
        let selection = Selection::read(ctx, cell, first.mode).await?;
        let bytes = Planner::new(&root, &toolchain, selection)
            .resolve(&self.0, &packages)
            .await?;
        let graph = super::units::Graph::parse(&bytes)?;
        let roots = graph
            .roots
            .iter()
            .map(|index| {
                let unit = &graph.units[*index];
                self.0
                    .iter()
                    .zip(&packages)
                    .find(|(request, package)| {
                        **package == unit.package_name
                            && request.entry.target.name() == unit.target.name
                            && (unit.target.kind == [request.entry.target.kind()]
                                || (request.entry.target.kind() == "lib"
                                    && super::libraries::is_library(&unit.target.kind)))
                    })
                    .map(|(request, _)| request.entry.clone())
                    .ok_or_else(|| unsupported("planner", "returned an unrequested root"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        if roots.is_empty() {
            return Ok(Arc::new(Plan {
                roots,
                source: String::new(),
            }));
        }
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
        let source = super::configured::render(
            &bytes,
            &root,
            cell.as_str(),
            &format!("{cell}//:__bsmr_rust"),
            execution,
        )?;
        Ok(Arc::new(Plan { roots, source }))
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
    type Value = bsmr_error::Result<Arc<Vec<Requested>>>;

    /// Resolve command roots from tracked metadata with the native pattern parser.
    async fn compute(&self, ctx: &mut DiceComputations, _: &CancellationContext) -> Self::Value {
        let invocation = ctx.compute(&Invocation).await?;
        let Some(invocation) = invocation.as_ref() else {
            return Ok(Arc::new(Vec::new()));
        };
        let patterns = crate::pattern::parse_from_cli::cargo_patterns(
            ctx,
            invocation.patterns(),
            invocation.working_dir(),
        )
        .await?;
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
    let requests = if selected.iter().any(|request| request.entry == *entry) {
        selected.as_ref().clone()
    } else {
        vec![Requested {
            entry: entry.clone(),
            origin: Origin::Explicit,
        }]
    };
    let plan = ctx.compute(&RustPlanKey(requests)).await??;
    if !plan.roots.contains(entry) {
        return Ok("load(\"@prelude//rust:cargo_outputs.bzl\", \"cargo_outputs\")\ncargo_outputs(name = \"root\", outputs = [], visibility = [\"PUBLIC\"])\n".to_owned());
    }
    let entries = &plan.roots;
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
    Ok(plan.source.clone())
}

/// Reject nonexistent or unsupported private packages before interpreter evaluation.
pub async fn validate_entry(
    ctx: &mut DiceComputations<'_>,
    entry: &Entry,
) -> bsmr_error::Result<()> {
    plan_file(ctx, entry).await.map(|_| ())
}
