//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Preserves Cargo library formats through native library subtargets.

use super::units::Graph;
use super::units::Mode;

/// Recognize only library formats represented by the qualified native rules.
pub(super) fn is_library(kinds: &[String]) -> bool {
    !kinds.is_empty()
        && kinds
            .iter()
            .all(|kind| matches!(kind.as_str(), "lib" | "rlib" | "cdylib" | "staticlib"))
}

/// Rust dependencies require a compiler-readable library, not only a C ABI output.
pub(super) fn has_rust_library(kinds: &[String]) -> bool {
    kinds
        .iter()
        .any(|kind| matches!(kind.as_str(), "lib" | "rlib"))
}

/// Require companion formats even when their unit is only an executable's dependency.
pub(super) fn outputs(graph: &Graph) -> Vec<String> {
    graph
        .units
        .iter()
        .enumerate()
        .filter(|(_, unit)| matches!(unit.mode, Mode::Build))
        .flat_map(|(index, unit)| {
            unit.target
                .kind
                .iter()
                .filter(|kind| matches!(kind.as_str(), "cdylib" | "staticlib"))
                .map(move |kind| format!(":unit_{index}[{}]", subtarget(kind)))
        })
        .collect()
}

/// Preserve the requested primary format for libraries without a Rust output.
pub(super) fn root(graph: &Graph, index: usize) -> String {
    let unit = &graph.units[index];
    let kinds = &unit.target.kind;
    if matches!(unit.mode, Mode::Build) && is_library(kinds) && !has_rust_library(kinds) {
        format!(":unit_{index}[{}]", subtarget(&kinds[0]))
    } else {
        format!(":unit_{index}")
    }
}

/// Cargo's default static library must remain linkable into position-independent executables.
fn subtarget(kind: &str) -> &str {
    match kind {
        "staticlib" => "staticlib_pic",
        other => other,
    }
}
