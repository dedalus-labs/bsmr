//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Gives each Cargo package one source artifact through existing native acquisition rules.

use std::collections::BTreeMap;
use std::path::Component;
use std::path::Path;

use serde_json::to_string as json;

use super::RustGraphError;
use super::units::Graph;
use super::units::SourceArtifact;
use super::unsupported;

/// Native source declarations and the package identities that own them.
pub(super) struct Sources<'a> {
    /// Rules which acquire immutable external source trees.
    pub rules: String,
    /// One artifact target for each Cargo package identity.
    pub targets: BTreeMap<&'a str, String>,
}

impl<'a> Sources<'a> {
    /// Share source acquisition across differently configured units of the same package.
    pub fn render(graph: &'a Graph, cell: &str) -> Result<Self, RustGraphError> {
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
                    format!(
                        "{cell}//{}:__bsmr_sources",
                        package.to_string_lossy().replace('\\', "/")
                    )
                }
                SourceArtifact::Archive {
                    url,
                    sha256,
                    size,
                    prefix,
                } => {
                    sources.rules.push_str(&format!(
                        "http_archive(name = {}, urls = [{}], sha256 = {}, size_bytes = {size}, strip_prefix = {}, type = \"tar.gz\", has_content_based_path = True)\n",
                        json(&name)?, json(url)?, json(sha256)?, json(prefix)?,
                    ));
                    format!(":{name}")
                }
                SourceArtifact::Git {
                    repository,
                    revision,
                    directory,
                } => {
                    let (rule, target) = git(repository, revision, directory, &name)?;
                    sources.rules.push_str(&rule);
                    target
                }
            };
            sources.targets.insert(&unit.package_id, target);
        }
        Ok(sources)
    }
}

/// Acquire a pinned repository and select its package without allowing path traversal.
fn git(
    repository: &str,
    revision: &str,
    directory: &Path,
    name: &str,
) -> Result<(String, String), RustGraphError> {
    if directory
        .components()
        .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(unsupported(
            repository,
            "Git package path escapes its source tree",
        ));
    }
    let directory = directory.to_string_lossy().replace('\\', "/");
    let subtargets: Vec<_> = (!directory.is_empty())
        .then_some(&directory)
        .into_iter()
        .collect();
    let rule = format!(
        "git_fetch(name = {}, repo = {}, rev = {}, sub_targets = {})\n",
        json(name)?,
        json(repository)?,
        json(revision)?,
        json(&subtargets)?,
    );
    let target = if directory.is_empty() {
        format!(":{name}")
    } else {
        format!(":{name}[{directory}]")
    };
    Ok((rule, target))
}
