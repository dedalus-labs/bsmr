//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Renders the toolchain targets that native frontends add to a package's own rules.

//! Native frontends contribute toolchains to a package without owning it.
//!
//! The package keeps whatever defines it (a Starlark build file or a native
//! manifest); the targets rendered here are appended after its own rules.

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::PathBuf;
use std::sync::Arc;

use allocative::Allocative;
use bsmr_core::fs::project_rel_path::ProjectRelativePath;
use bsmr_core::package::PackageLabel;
use bsmr_core::package::package_relative_path::PackageRelativePath;
use bsmr_fs::paths::abs_norm_path::AbsNormPathBuf;
use bsmr_fs::paths::file_name::FileName;
use dice::DiceComputations;
use dice::InjectedKey;
use dice::PagableValueSerialize;
use dice::ValueSerialize;
use dupe::Dupe;
use pagable::Pagable;
use pagable::pagable_typetag;
use serde::Deserialize;

use crate::dice::cells::HasCellResolver;
use crate::file_ops::dice::DiceFileComputations;
use crate::invocation_roots::home_bsmr_dir;
use crate::package_listing::PackageBuildSource;
use crate::package_listing::listing::PackageListing;

/// The committed Go SDK lock, at the project root, that `bsmr go toolchain` writes.
pub const GO_TOOLCHAIN_LOCK: &str = ".bsmr-go-toolchain.json";

/// The directory, beside the `toolchains//` package, where earlier releases copied the host SDK.
pub const GO_SDK_DIRECTORY: &str = ".bsmr-go-sdk";

/// Reports whether a directory name is an earlier release's Go acquisition, which is host state.
pub fn is_go_acquisition(name: &str) -> bool {
    name == GO_SDK_DIRECTORY
}

/// Verified Go SDK metadata files under the user root, by file name, read once per command.
///
/// `bsmr go toolchain` writes them outside every checkout, so the file watcher never sees a
/// change; injecting them per command lets a running daemon pick up a new acquisition.
#[derive(
    Clone,
    Dupe,
    Debug,
    Eq,
    PartialEq,
    Hash,
    Allocative,
    Pagable,
    derive_more::Display
)]
#[display("GoSdkMetadata")]
#[pagable_typetag(dice::DiceKeyDyn)]
pub struct GoSdkMetadata;

impl InjectedKey for GoSdkMetadata {
    type Value = Arc<BTreeMap<String, String>>;

    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }

    fn value_serialize() -> impl ValueSerialize<Value = Self::Value> {
        PagableValueSerialize::<Self::Value>::new()
    }
}

/// Returns the user-root directory holding one verified metadata file per Go SDK version.
pub fn go_sdk_metadata_directory() -> bsmr_error::Result<AbsNormPathBuf> {
    Ok(home_bsmr_dir()?
        .join(FileName::new("bootstrap")?)
        .join(FileName::new("go")?))
}

/// Reads every `go<version>.json` metadata file; an absent directory holds none.
pub fn capture_go_sdk_metadata() -> bsmr_error::Result<BTreeMap<String, String>> {
    let directory = go_sdk_metadata_directory()?;
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(error) => return Err(error.into()),
    };
    let mut metadata = BTreeMap::new();
    for entry in entries {
        let entry = entry?;
        if let Some(name) = entry.file_name().to_str()
            && name.starts_with("go")
            && name.ends_with(".json")
        {
            metadata.insert(name.to_owned(), fs::read_to_string(entry.path())?);
        }
    }
    Ok(metadata)
}

/// Reads the exact stable SDK pin from `go.mod`, which may hold one `toolchain goX.Y.Z`.
pub fn go_toolchain_pin(go_mod: &str) -> Result<Option<&str>, GoSdkMetadataError> {
    let mut pin = None;
    for line in go_mod.lines() {
        let mut words = line
            .split("//")
            .next()
            .unwrap_or_default()
            .split_whitespace();
        if words.next() != Some("toolchain") {
            continue;
        }
        let version = words
            .next()
            .and_then(|word| word.strip_prefix("go"))
            .ok_or(GoSdkMetadataError::Pin)?;
        let canonical = version.split('.').count() == 3
            && version.split('.').all(|part| {
                part.parse::<u64>()
                    .is_ok_and(|number| number.to_string() == part)
            });
        if !canonical || words.next().is_some() || pin.is_some() {
            return Err(GoSdkMetadataError::Pin);
        }
        pin = Some(version);
    }
    Ok(pin)
}

/// The fields that bind a metadata file to its writer, format, and pinned version.
#[derive(Deserialize)]
struct MetadataIdentity {
    /// Distinguishes Bessemer's metadata from another file with the same name.
    generated_by: String,
    /// Prevents a newer format from being read with older semantics.
    schema: u32,
    /// Must equal the `go.mod` pin that selected the file.
    version: String,
}

/// Fail-closed Go SDK selection errors during package evaluation.
#[derive(Debug, bsmr_error::Error)]
#[bsmr(tag = Input)]
pub enum GoSdkMetadataError {
    #[error("go.mod requires at most one `toolchain goX.Y.Z` directive with a stable release")]
    Pin,
    #[error("Go SDK metadata {0:?} is missing; run `bsmr go toolchain`")]
    Missing(PathBuf),
    #[error("Go SDK metadata {0:?} is invalid; run `bsmr go toolchain`")]
    Invalid(PathBuf),
}

/// Declares `cxx`, `python_bootstrap`, `genrule`, `test`, and `remote_test_execution`.
const NATIVE_TOOLS: &str = "load(\"@prelude//toolchains:native.bzl\", __bsmr_native_tools = \"native_tools\")\n__bsmr_native_tools()\n";

/// Renders the targets native frontends append to `package`.
///
/// The root package carries the shared native tools when a native frontend declares a
/// toolchain from it: a Cargo root beside its compiler, or a pinned `go.mod`. The Go SDK itself
/// joins whichever package `toolchains//` names. `bsmr init` aliases `toolchains` to the
/// root cell, so in its layout every contribution lands in one package, once.
pub async fn render(
    ctx: &mut DiceComputations<'_>,
    package: PackageLabel,
    listing: &PackageListing,
) -> bsmr_error::Result<String> {
    if !package.cell_relative_path().is_empty() {
        return Ok(String::new());
    }
    let mut source = String::new();
    let cargo_root = listing.build_source() == PackageBuildSource::Native
        && listing
            .get_file(PackageRelativePath::new("Cargo.toml")?)
            .is_some();
    let go = go_toolchain(ctx, package).await?;
    // A dedicated toolchain cell owns these names. Injecting them into a Go
    // source root as well collides with ordinary targets such as `:test`.
    let go_tools_here = !go.is_empty()
        && listing
            .get_file(PackageRelativePath::new("go.mod")?)
            .is_some();
    if cargo_root || go_tools_here {
        source.push_str(NATIVE_TOOLS);
    }
    source.push_str(&go);
    Ok(source)
}

/// Declares the pinned Go SDK when `package` is the one `toolchains//` names.
///
/// The root `go.mod` pin is read before the `toolchains` alias is resolved, so projects without
/// a pin never depend on that alias. The SDK and the bootstrap wrapper are action outputs.
async fn go_toolchain(
    ctx: &mut DiceComputations<'_>,
    package: PackageLabel,
) -> bsmr_error::Result<String> {
    let cells = ctx.get_cell_resolver().await?;
    let module = cells.get_cell_path(ProjectRelativePath::unchecked_new("go.mod"));
    let Some(module) = DiceFileComputations::read_file_if_exists(ctx, module.as_ref()).await?
    else {
        return Ok(String::new());
    };
    let Some(version) = go_toolchain_pin(&module)? else {
        return Ok(String::new());
    };
    if package.cell_name()
        != cells
            .root_cell_cell_alias_resolver()
            .resolve("toolchains")?
    {
        return Ok(String::new());
    }
    let name = format!("go{version}.json");
    let path = go_sdk_metadata_directory()?.join(FileName::new(&name)?);
    let metadata = ctx.compute(&GoSdkMetadata).await?;
    let lock = metadata
        .get(&name)
        .ok_or_else(|| GoSdkMetadataError::Missing(path.clone().into_path_buf()))?;
    if !serde_json::from_str::<MetadataIdentity>(lock).is_ok_and(|identity| {
        identity.generated_by == "bsmr go toolchain"
            && identity.schema == 1
            && identity.version == version
    }) {
        return Err(GoSdkMetadataError::Invalid(path.into_path_buf()).into());
    }
    Ok(format!(
        "load(\"@prelude//go/native:toolchain.bzl\", __bsmr_native_go_toolchains = \"native_go_toolchains\")\n__bsmr_native_go_toolchains(lock = {})\n",
        serde_json::to_string(&lock)?,
    ))
}
