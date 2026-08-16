//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Lowers Cargo manifests into BSMR's private cached-action graph.

use std::fmt::Write;

use bsmr_core::cells::paths::CellRelativePathBuf;
use bsmr_core::package::package_relative_path::PackageRelativePath;
use chrono::NaiveDate;
use serde::Deserialize;

use crate::package_listing::listing::PackageListing;

/// A Cargo manifest cannot be lowered without satisfying these invariants.
#[derive(Debug, bsmr_error::Error)]
#[bsmr(tag = Input)]
pub enum NativeCargoBuildError {
    /// The manifest must use Cargo's TOML schema.
    #[error("invalid Cargo manifest at `{root}`")]
    InvalidManifest {
        root: CellRelativePathBuf,
        #[source]
        source: toml::de::Error,
    },
    /// A package-local manifest needs a stable Cargo package identity.
    #[error("Cargo manifest at `{0}` must declare [package].name")]
    MissingPackageName(CellRelativePathBuf),
    /// The root manifest must define either a package or a workspace.
    #[error("root Cargo.toml must declare [package] or [workspace]")]
    MissingRootKind,
    /// Locked native builds require one authoritative dependency solution.
    #[error("native Cargo workspace is missing Cargo.lock")]
    MissingLockfile,
    /// Native Cargo builds require one exact rustup toolchain declaration.
    #[error("native Cargo builds require exactly one rust-toolchain or rust-toolchain.toml")]
    MissingRustToolchain,
    /// Multiple rustup toolchain files would make selection ambiguous.
    #[error("native Cargo builds cannot declare both rust-toolchain and rust-toolchain.toml")]
    AmbiguousRustToolchain,
    /// The rustup toolchain file must use its documented TOML schema.
    #[error("invalid rustup toolchain file")]
    InvalidToolchain(#[source] toml::de::Error),
    /// Mutable toolchain aliases cannot participate in a sound action key.
    #[error("Rust toolchain channel `{0}` is not exact")]
    InexactToolchain(String),
    /// BSMR's private targets must not collide with Cargo package targets.
    #[error("Cargo package target `{0}` is reserved by BSMR")]
    ReservedTargetName(String),
    /// Formatting private Starlark into a string should be infallible.
    #[error("failed to render native Cargo build graph")]
    Render(std::fmt::Error),
}

/// Returns the exact Rust channel declared by a rustup toolchain file.
pub fn parse_rust_toolchain(source: &str) -> Result<String, NativeCargoBuildError> {
    let file: RustToolchainFile =
        toml::from_str(source).map_err(NativeCargoBuildError::InvalidToolchain)?;
    let channel = file.toolchain.channel;
    if !is_exact_toolchain(&channel) {
        return Err(NativeCargoBuildError::InexactToolchain(channel));
    }
    Ok(channel)
}

/// Selects the project's one rustup toolchain file without compatibility fallbacks.
pub fn select_rust_toolchain_file(
    listing: &PackageListing,
) -> Result<&'static str, NativeCargoBuildError> {
    let files = ["rust-toolchain.toml", "rust-toolchain"]
        .into_iter()
        .filter(|file| {
            listing
                .get_file(PackageRelativePath::unchecked_new(file))
                .is_some()
        })
        .collect::<Vec<_>>();
    match files.as_slice() {
        [file] => Ok(file),
        [] => Err(NativeCargoBuildError::MissingRustToolchain),
        _ => Err(NativeCargoBuildError::AmbiguousRustToolchain),
    }
}

/// Renders the private Starlark bridge consumed by BSMR's build interpreter.
pub fn render_cargo_build_file(
    package_root: CellRelativePathBuf,
    manifest_source: &str,
    workspace_listing: &PackageListing,
    toolchain: &str,
) -> Result<String, NativeCargoBuildError> {
    let manifest: CargoManifest = toml::from_str(manifest_source).map_err(|source| {
        NativeCargoBuildError::InvalidManifest {
            root: package_root.clone(),
            source,
        }
    })?;
    let package_name = manifest
        .package
        .as_ref()
        .map(|package| package.name.as_str());
    if !package_root.is_empty() && package_name.is_none() {
        return Err(NativeCargoBuildError::MissingPackageName(package_root));
    }
    if package_root.is_empty() && package_name.is_none() && manifest.workspace.is_none() {
        return Err(NativeCargoBuildError::MissingRootKind);
    }
    let target_name = package_root
        .file_name()
        .map(|name| name.as_str())
        .or(package_name)
        .unwrap_or("workspace");
    if target_name == "__bsmr_cargo_workspace" {
        return Err(NativeCargoBuildError::ReservedTargetName(
            target_name.to_owned(),
        ));
    }

    let mut output = String::new();
    writeln!(
        output,
        "load(\"@prelude//cargo:defs.bzl\", \"cargo_build\", \"cargo_workspace\")\n"
    )
    .map_err(NativeCargoBuildError::Render)?;
    if package_root.is_empty() {
        render_workspace(
            &mut output,
            workspace_listing,
            package_name.is_some(),
            toolchain,
        )?;
    }
    let manifest_path = if package_root.is_empty() {
        "Cargo.toml".to_owned()
    } else {
        format!("{package_root}/Cargo.toml")
    };
    writeln!(output, "cargo_build(").map_err(NativeCargoBuildError::Render)?;
    writeln!(output, "    name = {target_name:?},").map_err(NativeCargoBuildError::Render)?;
    writeln!(output, "    manifest = {manifest_path:?},").map_err(NativeCargoBuildError::Render)?;
    writeln!(
        output,
        "    workspace = {:?},",
        if package_root.is_empty() {
            ":__bsmr_cargo_workspace"
        } else {
            "root//:__bsmr_cargo_workspace"
        }
    )
    .map_err(NativeCargoBuildError::Render)?;
    writeln!(output, "    visibility = [\"PUBLIC\"],").map_err(NativeCargoBuildError::Render)?;
    writeln!(output, ")").map_err(NativeCargoBuildError::Render)?;
    Ok(output)
}

#[derive(Debug, Deserialize)]
struct CargoManifest {
    package: Option<CargoPackage>,
    workspace: Option<toml::Value>,
}

#[derive(Debug, Deserialize)]
struct CargoPackage {
    name: String,
}

#[derive(Debug, Deserialize)]
struct RustToolchainFile {
    toolchain: RustToolchain,
}

#[derive(Debug, Deserialize)]
struct RustToolchain {
    channel: String,
}

/// Returns whether a rustup channel names immutable compiler bits.
fn is_exact_toolchain(channel: &str) -> bool {
    let stable = channel.split('.').collect::<Vec<_>>();
    if stable.len() == 3
        && stable
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return true;
    }
    channel
        .strip_prefix("nightly-")
        .is_some_and(|date| NaiveDate::parse_from_str(date, "%Y-%m-%d").is_ok())
}

/// Renders the cell's shared Cargo source tree and exact toolchain identity.
fn render_workspace(
    output: &mut String,
    listing: &PackageListing,
    root_is_package: bool,
    toolchain: &str,
) -> Result<(), NativeCargoBuildError> {
    if listing
        .get_file(PackageRelativePath::unchecked_new("Cargo.lock"))
        .is_none()
    {
        return Err(NativeCargoBuildError::MissingLockfile);
    }
    let manifest_roots = listing
        .files()
        .files()
        .filter(|path| path.file_name().is_some_and(|name| name == "Cargo.toml"))
        .filter_map(PackageRelativePath::parent)
        .filter(|path| !path.is_empty())
        .collect::<Vec<_>>();
    writeln!(output, "cargo_workspace(").map_err(NativeCargoBuildError::Render)?;
    writeln!(output, "    name = \"__bsmr_cargo_workspace\",")
        .map_err(NativeCargoBuildError::Render)?;
    writeln!(output, "    srcs = {{").map_err(NativeCargoBuildError::Render)?;
    for path in listing
        .files()
        .files()
        .filter(|path| is_cargo_workspace_file(path, &manifest_roots, root_is_package))
    {
        writeln!(output, "        {:?}: {:?},", path.as_str(), path.as_str())
            .map_err(NativeCargoBuildError::Render)?;
    }
    writeln!(output, "    }},").map_err(NativeCargoBuildError::Render)?;
    writeln!(output, "    toolchain = {toolchain:?},").map_err(NativeCargoBuildError::Render)?;
    writeln!(output, "    visibility = [\"PUBLIC\"],").map_err(NativeCargoBuildError::Render)?;
    writeln!(output, ")\n").map_err(NativeCargoBuildError::Render)
}

/// Selects Cargo metadata and every file owned by a discovered crate root.
fn is_cargo_workspace_file(
    path: &PackageRelativePath,
    manifest_roots: &[&PackageRelativePath],
    root_is_package: bool,
) -> bool {
    let path = path.as_str();
    if [
        ".bsmr",
        ".bsmr.local",
        ".git",
        "buck-out",
        "node_modules",
        "target",
    ]
    .into_iter()
    .any(|root| path == root || path.starts_with(&format!("{root}/")))
    {
        return false;
    }
    root_is_package
        || matches!(
            path,
            "Cargo.lock" | "Cargo.toml" | "rust-toolchain" | "rust-toolchain.toml"
        )
        || path.starts_with(".cargo/")
        || manifest_roots.iter().any(|root| {
            path.strip_prefix(root.as_str())
                .is_some_and(|suffix| suffix.starts_with('/'))
        })
}

#[cfg(test)]
mod tests {
    use bsmr_core::cells::paths::CellRelativePathBuf;

    use super::parse_rust_toolchain;
    use super::render_cargo_build_file;
    use super::select_rust_toolchain_file;
    use crate::package_listing::listing::PackageListing;
    use crate::package_listing::listing::testing::PackageListingExt;

    /// Exact stable and dated-nightly channels are immutable action-key inputs.
    #[test]
    fn invariant_toolchain_channel_is_exact() {
        assert_eq!(
            parse_rust_toolchain("[toolchain]\nchannel = \"1.94.1\"\n").unwrap(),
            "1.94.1"
        );
        assert_eq!(
            parse_rust_toolchain("[toolchain]\nchannel = \"nightly-2026-04-11\"\n").unwrap(),
            "nightly-2026-04-11"
        );
        assert!(parse_rust_toolchain("[toolchain]\nchannel = \"stable\"\n").is_err());
    }

    /// Toolchain selection is singular and never falls back to a mutable default.
    #[test]
    fn invariant_workspace_declares_exactly_one_toolchain_file() {
        let toml = PackageListing::testing_files(&["rust-toolchain.toml"]);
        assert_eq!(
            select_rust_toolchain_file(&toml).unwrap(),
            "rust-toolchain.toml"
        );
        assert!(select_rust_toolchain_file(&PackageListing::testing_files(&[])).is_err());
        assert!(
            select_rust_toolchain_file(&PackageListing::testing_files(&[
                "rust-toolchain",
                "rust-toolchain.toml",
            ]))
            .is_err()
        );
    }

    /// A virtual workspace owns one source tree and one conventional build target.
    #[test]
    fn invariant_virtual_workspace_renders_cached_cargo_action() {
        let listing = PackageListing::testing_files(&[
            ".bsmr",
            "Cargo.lock",
            "Cargo.toml",
            "rust-toolchain.toml",
            "packages/core/Cargo.toml",
            "packages/core/src/lib.rs",
            "packages/web/package.json",
            "packages/web/src/index.ts",
            "target/debug/stale",
        ]);

        let build = render_cargo_build_file(
            CellRelativePathBuf::unchecked_new(String::new()),
            "[workspace]\nmembers = [\"packages/core\"]\n",
            &listing,
            "1.94.1",
        )
        .unwrap();

        assert!(build.contains("name = \"__bsmr_cargo_workspace\""));
        assert!(build.contains("name = \"workspace\""));
        assert!(build.contains("manifest = \"Cargo.toml\""));
        assert!(build.contains("\"packages/core/src/lib.rs\""));
        assert!(!build.contains("packages/web/src/index.ts"));
        assert!(!build.contains("target/debug/stale"));
        assert!(!build.contains("\".bsmr\""));
    }

    /// A workspace member uses its path-shaped target and the root workspace provider.
    #[test]
    fn invariant_member_manifest_uses_root_workspace() {
        let build = render_cargo_build_file(
            CellRelativePathBuf::unchecked_new("packages/core".to_owned()),
            "[package]\nname = \"acme-core\"\nversion = \"0.1.0\"\n",
            &PackageListing::testing_empty(),
            "1.94.1",
        )
        .unwrap();

        assert!(build.contains("name = \"core\""));
        assert!(build.contains("manifest = \"packages/core/Cargo.toml\""));
        assert!(build.contains("workspace = \"root//:__bsmr_cargo_workspace\""));
        assert!(!build.contains("cargo_workspace("));
    }
}
