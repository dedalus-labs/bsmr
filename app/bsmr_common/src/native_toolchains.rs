//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Renders the toolchain targets that native frontends add to a package's own rules.

//! Native frontends contribute toolchains to a package without owning it.
//!
//! The package keeps whatever defines it (a Starlark build file or a native
//! manifest); the targets rendered here are appended after its own rules.

use bsmr_core::package::PackageLabel;
use bsmr_core::package::package_relative_path::PackageRelativePath;

use crate::package_listing::PackageBuildSource;
use crate::package_listing::listing::PackageListing;

/// Declares `cxx`, `python_bootstrap`, `genrule`, `test`, and `remote_test_execution`.
const NATIVE_TOOLS: &str = "load(\"@prelude//toolchains:native.bzl\", __bsmr_native_tools = \"native_tools\")\n__bsmr_native_tools()\n";

/// Renders the targets native frontends append to `package`.
///
/// A Cargo root carries the shared native tools beside its compiler. `bsmr init` aliases
/// `toolchains` to the root cell, which makes them the `toolchains//` defaults.
pub fn render(package: PackageLabel, listing: &PackageListing) -> bsmr_error::Result<String> {
    let cargo_root = package.cell_relative_path().is_empty()
        && listing.build_source() == PackageBuildSource::Native
        && listing
            .get_file(PackageRelativePath::new("Cargo.toml")?)
            .is_some();
    if cargo_root {
        Ok(NATIVE_TOOLS.to_owned())
    } else {
        Ok(String::new())
    }
}
