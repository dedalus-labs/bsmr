//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Parses and applies pnpm workspace package-selection patterns.

use std::collections::BTreeSet;

use bsmr_core::cells::paths::CellRelativePathBuf;
use globset::GlobBuilder;
use globset::GlobSet;
use globset::GlobSetBuilder;
use yaml_rust2::YamlLoader;

/// Failure to parse the package selection declared by `pnpm-workspace.yaml`.
#[derive(Debug, bsmr_error::Error)]
#[bsmr(tag = Input)]
pub enum PnpmWorkspaceError {
    /// The manifest is not valid YAML.
    #[error("invalid pnpm workspace YAML: {0}")]
    InvalidYaml(yaml_rust2::ScanError),
    /// A workspace manifest must contain exactly one YAML document.
    #[error("pnpm workspace manifest contains {0} YAML documents; expected exactly one")]
    DocumentCount(usize),
    /// The optional `packages` key must be a list when present.
    #[error("pnpm workspace `packages` value must be a list")]
    InvalidPackagePatterns,
    /// Every package selector must be a non-empty string.
    #[error("pnpm workspace package pattern at index {0} must be a non-empty string")]
    InvalidPackagePattern(usize),
    /// A selector cannot be represented by BSMR's deterministic glob engine.
    #[error("invalid pnpm workspace package pattern `{pattern}`: {error}")]
    InvalidPackageGlob {
        pattern: String,
        error: globset::Error,
    },
    /// An exclusion marker alone does not identify any package roots.
    #[error("pnpm workspace package pattern `{0}` has no glob after its exclusion marker")]
    EmptyPackageGlob(String),
    /// At least one inclusion is needed before exclusions can be applied.
    #[error("pnpm workspace manifest must declare at least one positive package pattern")]
    MissingPositivePackagePattern,
    /// The compiled selector set exceeded the glob engine's limits.
    #[error("unable to compile pnpm workspace package patterns: {0}")]
    InvalidPackageGlobSet(globset::Error),
}

/// The ordered package selectors declared by `pnpm-workspace.yaml`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PnpmWorkspace {
    patterns: Vec<String>,
}

impl PnpmWorkspace {
    /// Parses one strict `pnpm-workspace.yaml` document.
    pub fn parse(source: &str) -> Result<Self, PnpmWorkspaceError> {
        let documents =
            YamlLoader::load_from_str(source).map_err(PnpmWorkspaceError::InvalidYaml)?;
        if documents.len() != 1 {
            return Err(PnpmWorkspaceError::DocumentCount(documents.len()));
        }
        let packages = &documents[0]["packages"];
        if packages.is_badvalue() {
            return Ok(Self {
                patterns: Vec::new(),
            });
        }
        let Some(entries) = packages.as_vec() else {
            return Err(PnpmWorkspaceError::InvalidPackagePatterns);
        };
        let patterns = entries
            .iter()
            .enumerate()
            .map(|(index, entry)| {
                entry
                    .as_str()
                    .filter(|pattern| !pattern.is_empty())
                    .map(str::to_owned)
                    .ok_or(PnpmWorkspaceError::InvalidPackagePattern(index))
            })
            .collect::<Result<_, _>>()?;
        Ok(Self { patterns })
    }

    /// Selects candidate package roots with pnpm-style positive and negative globs.
    pub fn select_package_roots(
        &self,
        candidates: impl IntoIterator<Item = CellRelativePathBuf>,
    ) -> Result<Vec<CellRelativePathBuf>, PnpmWorkspaceError> {
        if self.patterns.is_empty() {
            return Ok(candidates
                .into_iter()
                .filter(|root| root.is_empty())
                .collect());
        }
        let (inclusions, exclusions) = compile_package_patterns(&self.patterns)?;
        Ok(candidates
            .into_iter()
            .filter(|root| {
                root.is_empty()
                    || (inclusions.is_match(root.as_str()) && !exclusions.is_match(root.as_str()))
            })
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect())
    }
}

/// Compiles pnpm selectors into independent inclusion and exclusion sets.
fn compile_package_patterns(patterns: &[String]) -> Result<(GlobSet, GlobSet), PnpmWorkspaceError> {
    let mut inclusions = GlobSetBuilder::new();
    let mut exclusions = GlobSetBuilder::new();
    let mut inclusion_count = 0;
    for raw_pattern in patterns {
        let (builder, pattern) = match raw_pattern.strip_prefix('!') {
            Some(pattern) => (&mut exclusions, pattern),
            None => {
                inclusion_count += 1;
                (&mut inclusions, raw_pattern.as_str())
            }
        };
        if pattern.is_empty() {
            return Err(PnpmWorkspaceError::EmptyPackageGlob(raw_pattern.clone()));
        }
        let glob = GlobBuilder::new(pattern)
            .literal_separator(true)
            .backslash_escape(false)
            .build()
            .map_err(|error| PnpmWorkspaceError::InvalidPackageGlob {
                pattern: raw_pattern.clone(),
                error,
            })?;
        builder.add(glob);
    }
    if inclusion_count == 0 {
        return Err(PnpmWorkspaceError::MissingPositivePackagePattern);
    }
    Ok((
        inclusions
            .build()
            .map_err(PnpmWorkspaceError::InvalidPackageGlobSet)?,
        exclusions
            .build()
            .map_err(PnpmWorkspaceError::InvalidPackageGlobSet)?,
    ))
}
