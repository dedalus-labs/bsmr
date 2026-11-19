//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Declares the workspace and read-only Git state consumed by first-party build scripts.

use std::collections::BTreeMap;
use std::collections::VecDeque;

use bsmr_core::cells::paths::CellRelativePathBuf;
use bsmr_core::package::PackageLabel;
use bsmr_core::package::package_relative_path::PackageRelativePath;
use dice::DiceComputations;

use crate::dice::cells::HasCellResolver;
use crate::package_listing::PackageBuildSource;
use crate::package_listing::dice::DicePackageListingResolver;

/// Assemble every native package with this command's captured Git inputs.
pub async fn render(
    ctx: &mut DiceComputations<'_>,
    root: PackageLabel,
) -> bsmr_error::Result<String> {
    if !ctx
        .get_cell_resolver()
        .await?
        .is_root_cell(root.cell_name())
    {
        return Err(super::unsupported(
            &root.to_string(),
            "checkout identity belongs to the project root",
        )
        .into());
    }
    let mut packages = BTreeMap::new();
    let mut unavailable = Vec::new();
    let mut pending = VecDeque::from([String::new()]);
    while let Some(path) = pending.pop_front() {
        let label = PackageLabel::new(
            root.cell_name(),
            &CellRelativePathBuf::try_from(path.clone())?,
        )?;
        let listing = DicePackageListingResolver(ctx)
            .resolve_package_listing(label)
            .await?;
        if !matches!(listing.build_source(), PackageBuildSource::Native) {
            unavailable.push(path);
            continue;
        }
        packages.insert(
            path.clone(),
            format!("{}//{path}:__bsmr_checkout_sources", root.cell_name()),
        );
        for nested in listing.subpackages_within(PackageRelativePath::empty()) {
            pending.push_back(if path.is_empty() {
                nested.to_string()
            } else {
                format!("{path}/{nested}")
            });
        }
    }
    let files = ctx.compute(&super::git::GitInputs).await?;
    let git: BTreeMap<_, _> = files
        .iter()
        .map(|(path, file)| (path, (&file.url, &file.sha256, file.size)))
        .collect();
    let rule = format!(
        "load(\"@prelude//rust:checkout.bzl\", \"cargo_checkout\")\n\
         cargo_checkout(name = \"__bsmr_checkout\", packages = {}, git = {}, unavailable = {}, visibility = [\"PUBLIC\"])\n",
        serde_json::to_string(&packages)?,
        serde_json::to_string(&git)?,
        serde_json::to_string(&unavailable)?,
    );
    Ok(rule)
}
