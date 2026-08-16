//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Reads frozen pnpm importer resolutions needed to disambiguate workspace edges.

use std::collections::BTreeMap;

use bsmr_core::cells::paths::CellRelativePathBuf;
use yaml_rust2::YamlLoader;

use super::DependencySection;

/// A frozen lockfile must expose typed importer resolutions.
#[derive(Debug, bsmr_error::Error)]
#[bsmr(tag = Input)]
enum PnpmLockError {
    /// The lockfile must be syntactically valid YAML.
    #[error("invalid pnpm lockfile YAML: {0}")]
    InvalidYaml(yaml_rust2::ScanError),
    /// pnpm lockfiles contain exactly one YAML document.
    #[error("pnpm lockfile contains {0} YAML documents; expected exactly one")]
    DocumentCount(usize),
    /// Native graph resolution requires the importer map.
    #[error("pnpm lockfile is missing its `importers` map")]
    MissingImporters,
    /// Importer keys are normalized project-relative paths.
    #[error("pnpm lockfile contains invalid importer path `{0}`")]
    InvalidImporterPath(String),
    /// Dependency sections and entries must use pnpm's mapping schema.
    #[error("pnpm lockfile importer `{importer}` has invalid `{section}` entries")]
    InvalidSection { importer: String, section: String },
    /// Every frozen importer dependency records its manifest specifier and resolution.
    #[error(
        "pnpm lockfile importer `{importer}` has invalid `{section}` resolution for `{dependency}`"
    )]
    InvalidResolution {
        importer: String,
        section: String,
        dependency: String,
    },
}

#[derive(Debug)]
struct LockedDependency {
    specifier: String,
    version: String,
}

/// One frozen pnpm lockfile indexed by importer, section, and dependency name.
#[derive(Debug)]
pub(super) struct PnpmLock {
    resolutions: BTreeMap<(CellRelativePathBuf, DependencySection, String), LockedDependency>,
}

impl PnpmLock {
    /// Parses the importer resolutions from one pnpm lockfile.
    pub(super) fn parse(source: &str) -> bsmr_error::Result<Self> {
        let documents = YamlLoader::load_from_str(source).map_err(PnpmLockError::InvalidYaml)?;
        if documents.len() != 1 {
            return Err(PnpmLockError::DocumentCount(documents.len()).into());
        }
        let Some(importers) = documents[0]["importers"].as_hash() else {
            return Err(PnpmLockError::MissingImporters.into());
        };
        let mut resolutions = BTreeMap::new();
        for (importer, value) in importers {
            let Some(importer) = importer.as_str() else {
                return Err(PnpmLockError::InvalidImporterPath(format!("{importer:?}")).into());
            };
            let normalized = if importer == "." { "" } else { importer };
            let root = CellRelativePathBuf::try_from(normalized.to_owned())
                .map_err(|_| PnpmLockError::InvalidImporterPath(importer.to_owned()))?;
            for (section, key) in lock_sections() {
                let entries = &value[key];
                if entries.is_badvalue() {
                    continue;
                }
                let Some(entries) = entries.as_hash() else {
                    return Err(PnpmLockError::InvalidSection {
                        importer: importer.to_owned(),
                        section: key.to_owned(),
                    }
                    .into());
                };
                for (dependency, resolution) in entries {
                    let Some(dependency) = dependency.as_str() else {
                        return Err(invalid_resolution(importer, key, dependency).into());
                    };
                    let Some(specifier) = resolution["specifier"].as_str() else {
                        return Err(invalid_resolution(importer, key, dependency).into());
                    };
                    let Some(version) = resolution["version"].as_str() else {
                        return Err(invalid_resolution(importer, key, dependency).into());
                    };
                    resolutions.insert(
                        (root.clone(), section, dependency.to_owned()),
                        LockedDependency {
                            specifier: specifier.to_owned(),
                            version: version.to_owned(),
                        },
                    );
                }
            }
        }
        Ok(Self { resolutions })
    }

    /// Returns whether one importer dependency is frozen to the expected workspace root.
    pub(super) fn resolves_to_workspace(
        &self,
        importer: &CellRelativePathBuf,
        section: DependencySection,
        dependency: &str,
        specifier: &str,
        target: &CellRelativePathBuf,
    ) -> bool {
        self.resolution(importer, section, dependency, specifier)
            .and_then(|version| version.strip_prefix("link:"))
            .and_then(|link| resolve_link(importer, link))
            .is_some_and(|root| root == target.as_str())
    }

    /// Returns whether one importer dependency is frozen to the registry.
    pub(super) fn resolves_to_registry(
        &self,
        importer: &CellRelativePathBuf,
        section: DependencySection,
        dependency: &str,
        specifier: &str,
    ) -> bool {
        self.resolution(importer, section, dependency, specifier)
            .is_some_and(|version| !version.starts_with("link:"))
    }

    /// Returns one resolution only when the lockfile matches the manifest declaration.
    fn resolution(
        &self,
        importer: &CellRelativePathBuf,
        section: DependencySection,
        dependency: &str,
        specifier: &str,
    ) -> Option<&str> {
        self.resolutions
            .get(&(importer.clone(), section, dependency.to_owned()))
            .filter(|locked| locked.specifier == specifier)
            .map(|locked| locked.version.as_str())
    }
}

/// Maps package.json dependency sections to pnpm importer keys.
fn lock_sections() -> [(DependencySection, &'static str); 3] {
    [
        (DependencySection::Dependency, "dependencies"),
        (DependencySection::DevDependency, "devDependencies"),
        (
            DependencySection::OptionalDependency,
            "optionalDependencies",
        ),
    ]
}

/// Constructs one stable schema diagnostic for a malformed importer entry.
fn invalid_resolution(
    importer: &str,
    section: &str,
    dependency: impl std::fmt::Debug,
) -> PnpmLockError {
    PnpmLockError::InvalidResolution {
        importer: importer.to_owned(),
        section: section.to_owned(),
        dependency: format!("{dependency:?}"),
    }
}

/// Resolves pnpm's importer-relative `link:` path without touching the filesystem.
fn resolve_link(importer: &CellRelativePathBuf, link: &str) -> Option<String> {
    let mut components = importer
        .as_str()
        .split('/')
        .filter(|component| !component.is_empty())
        .collect::<Vec<_>>();
    for component in link.split('/') {
        match component {
            "" | "." => {}
            ".." => {
                components.pop()?;
            }
            component if component.contains('\\') => return None,
            component => components.push(component),
        }
    }
    Some(components.join("/"))
}
