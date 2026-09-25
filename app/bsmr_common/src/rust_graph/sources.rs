//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Gives each Cargo package one source artifact through existing native acquisition rules.

use std::collections::BTreeMap;

use serde_json::to_string as json;

use super::RustGraphError;
use super::units::Graph;
use super::units::SourceArtifact;

/// Native source declarations and the package identities that own them.
pub(super) struct Sources<'a> {
    /// Rules which acquire immutable external source trees.
    pub rules: String,
    /// One artifact target for each Cargo package identity.
    pub targets: BTreeMap<&'a str, Source>,
}

/// A package projection retains the source tree that owns its relative links.
pub(super) struct Source {
    /// Complete source-tree artifact, retained by each consuming action.
    pub root: String,
    /// Package artifact selected inside the tree.
    pub package: String,
}

impl<'a> Sources<'a> {
    /// Share source acquisition across differently configured units of the same package.
    pub fn render(graph: &'a Graph, cell: &str) -> Result<Self, RustGraphError> {
        let boundaries = graph
            .workspace_packages
            .iter()
            .map(|path| {
                path.strip_prefix(&graph.workspace_root)
                    .map(|path| path.to_string_lossy().replace('\\', "/"))
                    .map_err(|_| RustGraphError::Outside(path.clone()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut sources = Self {
            rules: String::new(),
            targets: BTreeMap::new(),
        };
        for unit in &graph.units {
            if sources.targets.contains_key(unit.package_id.as_str()) {
                continue;
            }
            let name = format!("source_{}", sources.targets.len());
            let target = match &unit.source.artifact {
                SourceArtifact::Workspace => {
                    let package = unit
                        .source
                        .root
                        .strip_prefix(&graph.workspace_root)
                        .map_err(|_| RustGraphError::Outside(unit.source.root.clone()))?;
                    sources.rules.push_str(&format!(
                        "load(\"@prelude//rust:checkout.bzl\", \"cargo_source\")\n\
                         cargo_source(name = {}, checkout = {}, package = {}, boundaries = {})\n",
                        json(&name)?,
                        json(&format!("{cell}//:__bsmr_checkout"))?,
                        json(&package.to_string_lossy().replace('\\', "/"))?,
                        json(&boundaries)?,
                    ));
                    Source {
                        root: format!(":{name}"),
                        package: format!(":{name}[package]"),
                    }
                }
                SourceArtifact::Archive {
                    url,
                    sha256,
                    size,
                    prefix,
                    package,
                } => {
                    sources.rules.push_str(&format!(
                        "http_archive(name = {}, urls = [{}], sha256 = {}, size_bytes = {size}, strip_prefix = {}, sub_targets = {{\"package\": [{}]}}, type = \"tar.gz\", has_content_based_path = True)\n",
                        json(&name)?, json(url)?, json(sha256)?, json(prefix)?, json(package)?,
                    ));
                    Source {
                        root: format!(":{name}"),
                        package: format!(":{name}[package]"),
                    }
                }
            };
            sources.targets.insert(&unit.package_id, target);
        }
        Ok(sources)
    }
}
