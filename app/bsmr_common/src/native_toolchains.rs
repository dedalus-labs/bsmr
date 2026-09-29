//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Renders the toolchain targets that native frontends add to a package's own rules.

//! Native frontends contribute toolchains to a package without owning it.
//!
//! The package keeps whatever defines it (a Starlark build file or a native
//! manifest); the targets rendered here are appended after its own rules.

use bsmr_core::fs::project_rel_path::ProjectRelativePath;
use bsmr_core::package::PackageLabel;
use bsmr_core::package::package_relative_path::PackageRelativePath;
use dice::DiceComputations;

use crate::dice::cells::HasCellResolver;
use crate::file_ops::dice::DiceFileComputations;
use crate::package_listing::PackageBuildSource;
use crate::package_listing::listing::PackageListing;

/// The committed Go SDK lock, at the project root, that `bsmr go toolchain` writes.
pub const GO_TOOLCHAIN_LOCK: &str = ".bsmr-go-toolchain.json";

/// The ignored directory, beside the `toolchains//` package, that holds the acquired host SDK.
pub const GO_SDK_DIRECTORY: &str = ".bsmr-go-sdk";

/// The ignored directory, beside the SDK, that holds the bootstrap wrapper it compiled.
pub const GO_TOOLS_DIRECTORY: &str = ".bsmr-go-tools";

/// Reports whether a directory name is one of the Go acquisitions, which are local host state.
pub fn is_go_acquisition(name: &str) -> bool {
    name == GO_SDK_DIRECTORY || name == GO_TOOLS_DIRECTORY
}

/// Declares `cxx`, `python_bootstrap`, `genrule`, `test`, and `remote_test_execution`.
const NATIVE_TOOLS: &str = "load(\"@prelude//toolchains:native.bzl\", __bsmr_native_tools = \"native_tools\")\n__bsmr_native_tools()\n";

/// Renders the targets native frontends append to `package`.
///
/// The root package carries the shared native tools when a native frontend declares a
/// toolchain from it: a Cargo root beside its compiler, or the Go lock. The Go SDK itself
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
    let go_root = listing
        .get_file(PackageRelativePath::new(GO_TOOLCHAIN_LOCK)?)
        .is_some();
    if cargo_root || go_root {
        source.push_str(NATIVE_TOOLS);
    }
    source.push_str(&go_toolchain(ctx, package, listing).await?);
    Ok(source)
}

/// Declares the locked Go SDK when `package` is the one `toolchains//` names.
///
/// The lock is read before the `toolchains` alias is resolved, so projects without Go never
/// depend on that alias. The SDK and bootstrap wrapper are ignored local state; until
/// `bsmr go toolchain` installs them, `go` and `go_bootstrap` fail only when a build uses them.
async fn go_toolchain(
    ctx: &mut DiceComputations<'_>,
    package: PackageLabel,
    listing: &PackageListing,
) -> bsmr_error::Result<String> {
    let cells = ctx.get_cell_resolver().await?;
    let lock = cells.get_cell_path(ProjectRelativePath::unchecked_new(GO_TOOLCHAIN_LOCK));
    let Some(lock) = DiceFileComputations::read_file_if_exists(ctx, lock.as_ref()).await? else {
        return Ok(String::new());
    };
    if package.cell_name()
        != cells
            .root_cell_cell_alias_resolver()
            .resolve("toolchains")?
    {
        return Ok(String::new());
    }
    let acquired = listing
        .get_dir(PackageRelativePath::new(GO_SDK_DIRECTORY)?)
        .is_some()
        && listing
            .get_file(PackageRelativePath::new(&format!(
                "{GO_TOOLS_DIRECTORY}/go_wrapper"
            ))?)
            .is_some();
    Ok(format!(
        "load(\"@prelude//go/native:toolchain.bzl\", __bsmr_native_go_toolchains = \"native_go_toolchains\")\n__bsmr_native_go_toolchains(lock = {}, acquired = {})\n",
        serde_json::to_string(&lock)?,
        if acquired { "True" } else { "False" },
    ))
}
