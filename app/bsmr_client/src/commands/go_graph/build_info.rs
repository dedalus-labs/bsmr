//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Derives the module lines Go records in a main package's `debug.BuildInfo`.

//! Mirrors the module half of `setBuildInfo` in `cmd/go/internal/load/pkg.go`.
//!
//! The main package's module becomes `mod`; every other module reachable through
//! production imports becomes one `dep`, ordered by path; a replaced module is
//! followed by its `=>` line. Lines use the tab-separated text of
//! `debug.BuildInfo.String`, which the linker embeds verbatim.
//!
//! Sums are always empty. `go build -mod=vendor` records none, and synchronization
//! admits only vendored modules, replaced modules, and repository-local workspace
//! modules, none of which `go build` gives a sum. Build settings belong to the
//! prelude, which knows the configured platform at analysis time.

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use super::metadata::ListedModule;
use super::metadata::ListedPackage;
use crate::commands::go_graph_error::GoGraphError;

/// Returns `mod`, `dep`, and `=>` lines for a main package's linked module graph.
pub(super) fn module_lines(
    package: &ListedPackage,
    listed: &BTreeMap<String, ListedPackage>,
) -> Result<Vec<String>, GoGraphError> {
    let mut lines = Vec::new();
    let Some(main) = &package.module else {
        return Ok(lines);
    };
    push_module(&mut lines, "mod", main);
    for dependency in dependency_modules(package, &main.path, listed)?.into_values() {
        push_module(&mut lines, "dep", dependency);
    }
    Ok(lines)
}

/// Collects each non-main module that supplies a transitively imported package.
fn dependency_modules<'a>(
    package: &'a ListedPackage,
    main_path: &str,
    listed: &'a BTreeMap<String, ListedPackage>,
) -> Result<BTreeMap<&'a str, &'a ListedModule>, GoGraphError> {
    let mut modules = BTreeMap::new();
    let mut visited = BTreeSet::new();
    let mut pending = vec![package];
    while let Some(importer) = pending.pop() {
        for import in importer
            .imports
            .iter()
            .filter(|import| import.as_str() != "C")
        {
            if !visited.insert(import.as_str()) {
                continue;
            }
            let imported = listed
                .get(import)
                .ok_or_else(|| GoGraphError::MissingDependency {
                    package: importer.import_path.clone(),
                    dependency: import.clone(),
                })?;
            if let Some(module) = imported
                .module
                .as_ref()
                .filter(|module| module.path != main_path)
            {
                modules.insert(module.path.as_str(), module);
            }
            pending.push(imported);
        }
    }
    Ok(modules)
}

/// Appends one module in `debug.BuildInfo.String` form, with its replacement if any.
fn push_module(lines: &mut Vec<String>, word: &str, module: &ListedModule) {
    // Go reports main and workspace modules without a version and records them as `(devel)`.
    let version = if module.version.is_empty() {
        "(devel)"
    } else {
        &module.version
    };
    match &module.replace {
        None => lines.push(format!("{word}\t{}\t{version}\t", module.path)),
        Some(replacement) => {
            lines.push(format!("{word}\t{}\t{version}", module.path));
            push_module(lines, "=>", replacement);
        }
    }
}
