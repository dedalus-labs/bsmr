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

use super::render;
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
        let mut rules: BTreeMap<_, _> =
            render(&metadata, &root, &format!("{}//:__bsmr_rust", self.0))?
                .into_iter()
                .map(|(path, text)| {
                    (
                        path.parent()
                            .unwrap()
                            .strip_prefix(&root)
                            .unwrap()
                            .to_string_lossy()
                            .replace('\\', "/"),
                        text.replace("root//", &format!("{}//", self.0)),
                    )
                })
                .collect();
        rules
            .entry(String::new())
            .or_default()
            .push_str(&toolchain.rules);
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
        if relative.ends_with(".cargo/config") || relative.ends_with(".cargo/config.toml") {
            return Err(unsupported(relative, "Cargo configuration").into());
        }
        let manifest = Path::new(relative)
            .file_name()
            .is_some_and(|f| f == "Cargo.toml");
        if !manifest
            && relative != "Cargo.lock"
            && relative != "rust-toolchain.toml"
            && !relative.ends_with(".rs")
        {
            return Ok(());
        }
        let destination = root.join(relative);
        std::fs::create_dir_all(destination.parent().expect("snapshot file has parent"))?;
        let source = if manifest || relative == "Cargo.lock" || relative == "rust-toolchain.toml" {
            DiceFileComputations::read_file(
                ctx,
                CellPath::new(self.0, CellRelativePathBuf::try_from(relative.to_owned())?).as_ref(),
            )
            .await
            .without_package_context_information()?
        } else {
            String::new()
        };
        if manifest {
            validate_manifest(relative, &source)?;
        }
        std::fs::write(destination, source)?;
        Ok(())
    }
}

/// Reject manifest semantics not represented by the native graph.
fn validate_manifest(relative: &str, source: &str) -> bsmr_error::Result<()> {
    let value: toml::Value = toml::from_str(source)?;
    for table in ["lib", "bin", "test", "bench", "example"] {
        if let Some(targets) = value.get(table) {
            let targets = targets
                .as_array()
                .map(Vec::as_slice)
                .unwrap_or(std::slice::from_ref(targets));
            if targets.iter().any(|t| t.get("harness").is_some()) {
                return Err(unsupported(relative, "custom test harness").into());
            }
        }
    }
    for key in ["profile", "lints"] {
        if value.get(key).is_some() || value.get("workspace").and_then(|w| w.get(key)).is_some() {
            return Err(unsupported(relative, key).into());
        }
    }
    Ok(())
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
        tokio::process::Command::new(&toolchain.cargo)
            .args(["metadata", "--format-version=1", "--frozen"])
            .current_dir(root)
            .env_clear()
            .env("CARGO_HOME", cargo_home.path())
            .env("PATH", toolchain.cargo.parent().expect("Cargo has parent"))
            .env("RUSTC", &toolchain.rustc)
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

/// Return the inferred source for one package without writing build files.
pub async fn build_file(
    ctx: &mut DiceComputations<'_>,
    package: PackageLabel,
) -> bsmr_error::Result<String> {
    let rules = ctx.compute(&RustGraphKey(package.cell_name())).await??;
    rules
        .get(package.as_cell_path().path().as_str())
        .cloned()
        .ok_or_else(|| {
            unsupported(
                &package.to_string(),
                "package absent from Cargo's resolved workspace",
            )
            .into()
        })
}
