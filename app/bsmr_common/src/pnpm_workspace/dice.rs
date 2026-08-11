//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Caches one normalized pnpm workspace graph per BSMR cell.

use std::future::Future;
use std::sync::Arc;

use async_trait::async_trait;
use bsmr_core::cells::cell_path::CellPath;
use bsmr_core::cells::name::CellName;
use bsmr_core::cells::paths::CellRelativePathBuf;
use bsmr_core::package::PackageLabel;
use bsmr_core::package::package_relative_path::PackageRelativePath;
use bsmr_fs::paths::forward_rel_path::ForwardRelativePath;
use dice::DiceComputations;
use dice::Key;
use dice::OkPagableValueSerialize;
use dice::ValueSerialize;
use dice_futures::cancellation::CancellationContext;
use futures::FutureExt;
use pagable::Pagable;
use pagable::pagable_typetag;

use super::PnpmWorkspace;
use super::WorkspaceGraph;
use super::WorkspacePackage;
use crate::file_ops::dice::DiceFileComputations;
use crate::file_ops::error::FileReadErrorContext;
use crate::package_listing::dice::DicePackageListingResolver;

/// A pnpm graph requires an authoritative root package manifest.
#[derive(Debug, bsmr_error::Error)]
#[bsmr(tag = Input)]
enum PnpmWorkspaceGraphError {
    /// Every pnpm workspace has one root package manifest.
    #[error("pnpm workspace root is missing package.json")]
    MissingRootManifest,
}

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
#[display("PnpmWorkspaceGraphKey({})", _0)]
#[pagable_typetag(dice::DiceKeyDyn)]
struct PnpmWorkspaceGraphKey(CellName);

#[async_trait]
impl Key for PnpmWorkspaceGraphKey {
    type Value = bsmr_error::Result<Arc<WorkspaceGraph>>;

    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        _cancellations: &CancellationContext,
    ) -> Self::Value {
        let root_path = CellRelativePathBuf::unchecked_new(String::new());
        let root = PackageLabel::new(self.0, &root_path)?;
        let listing = DicePackageListingResolver(ctx)
            .resolve_package_listing(root)
            .await?;
        let candidates = listing
            .files()
            .files()
            .filter(|path| path.file_name().is_some_and(|name| name == "package.json"))
            .map(|path| {
                CellRelativePathBuf::try_from(
                    path.parent()
                        .map_or("", PackageRelativePath::as_str)
                        .to_owned(),
                )
            })
            .collect::<bsmr_error::Result<Vec<_>>>()?;
        if !candidates.iter().any(|path| path.is_empty()) {
            return Err(PnpmWorkspaceGraphError::MissingRootManifest.into());
        }

        let workspace_path = PackageRelativePath::new("pnpm-workspace.yaml")?;
        let roots = match listing.get_file(workspace_path) {
            Some(path) => {
                let source = DiceFileComputations::read_file(
                    ctx,
                    CellPath::new(self.0, CellRelativePathBuf::unchecked_new(path.to_string()))
                        .as_ref(),
                )
                .await
                .without_package_context_information()?;
                PnpmWorkspace::parse(&source)?.select_package_roots(candidates)?
            }
            None => vec![CellRelativePathBuf::unchecked_new(String::new())],
        };
        let packages = ctx
            .try_compute_join(roots, |ctx, root| {
                async move {
                    let manifest_path =
                        root.join(ForwardRelativePath::unchecked_new("package.json"));
                    let source = DiceFileComputations::read_file(
                        ctx,
                        CellPath::new(self.0, manifest_path).as_ref(),
                    )
                    .await
                    .without_package_context_information()?;
                    bsmr_error::Ok(WorkspacePackage::parse(root, &source)?)
                }
                .boxed()
            })
            .await?;
        Ok(Arc::new(WorkspaceGraph::build(packages)?))
    }

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        matches!((x, y), (Ok(x), Ok(y)) if x == y)
    }

    fn value_serialize() -> impl ValueSerialize<Value = Self::Value> {
        OkPagableValueSerialize::<Self::Value>::new()
    }
}

/// Provides the DICE-cached native pnpm workspace graph.
pub trait HasPnpmWorkspaceGraph {
    /// Returns the normalized graph for one cell.
    fn get_pnpm_workspace_graph(
        &mut self,
        cell: CellName,
    ) -> impl Future<Output = bsmr_error::Result<Arc<WorkspaceGraph>>>;
}

impl HasPnpmWorkspaceGraph for DiceComputations<'_> {
    async fn get_pnpm_workspace_graph(
        &mut self,
        cell: CellName,
    ) -> bsmr_error::Result<Arc<WorkspaceGraph>> {
        self.compute(&PnpmWorkspaceGraphKey(cell)).await?
    }
}
