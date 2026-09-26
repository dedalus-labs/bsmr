//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Exposes Cargo-discovered entrypoints without resolving unrelated compilation units.

use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;

use bsmr_core::cells::name::CellName;
use bsmr_core::cells::paths::CellRelativePathBuf;
use bsmr_core::package::PackageLabel;
use bsmr_core::pattern::pattern::ParsedPattern;
use bsmr_core::pattern::pattern_type::ConfiguredProvidersPatternExtra;
use serde::Deserialize;
use serde_json::to_string as json;

use super::RustGraphError;
use super::entry::Entry;
use super::entry::Mode;
use super::entry::Target;
use super::libraries;
use super::unsupported;

#[derive(Deserialize)]
struct Metadata {
    /// Cargo metadata protocol version.
    version: u32,
    /// Captured workspace whose paths may appear in the catalog.
    workspace_root: PathBuf,
    /// Workspace members discovered without resolving their dependencies.
    packages: Vec<Package>,
}

#[derive(Deserialize)]
struct Package {
    /// Name used by Cargo's package selector.
    name: String,
    /// Manifest that defines the physical package boundary.
    manifest_path: PathBuf,
    /// Cargo's discovered targets, including non-entrypoint helper targets.
    targets: Vec<CargoTarget>,
}

#[derive(Deserialize)]
struct CargoTarget {
    /// Manifest-declared or conventionally discovered source entrypoint.
    src_path: PathBuf,
    /// Original target name, before Rust identifier normalization.
    name: String,
    /// Cargo target classification.
    kind: Vec<String>,
    /// Whether Cargo enables this target's unit test harness.
    test: bool,
}

/// Cargo metadata discovers explicit paths even before checking that they exist.
pub(super) fn entrypoints(bytes: &[u8]) -> Result<Vec<PathBuf>, RustGraphError> {
    let metadata: Metadata = serde_json::from_slice(bytes)?;
    Ok(metadata
        .packages
        .into_iter()
        .flat_map(|package| package.targets)
        .map(|target| target.src_path)
        .collect())
}

/// Render public aliases and source trees. Compilation planning remains demand-driven.
pub fn render(
    bytes: &[u8],
    root: &Path,
    cell: &str,
) -> Result<BTreeMap<String, String>, RustGraphError> {
    let metadata: Metadata = serde_json::from_slice(bytes)?;
    if metadata.version != 1 || metadata.workspace_root != root {
        return Err(unsupported(
            "workspace",
            "metadata version or workspace root mismatch",
        ));
    }
    metadata
        .packages
        .iter()
        .map(|package| {
            let path = package
                .manifest_path
                .parent()
                .expect("Cargo manifest has a parent")
                .strip_prefix(root)
                .map_err(|_| RustGraphError::Outside(package.manifest_path.clone()))?;
            let path = path.to_string_lossy().replace('\\', "/");
            Ok((path.clone(), package.render(&path, cell)?))
        })
        .collect()
}

impl Package {
    /// Keep unsupported helper targets out of the public catalog, not out of dependency plans.
    fn render(&self, path: &str, cell: &str) -> Result<String, RustGraphError> {
        let mut source = sources().to_owned();
        let mut names = Vec::new();
        let integration_tests: Vec<_> = self
            .targets
            .iter()
            .filter(|target| target.kind == ["test"] && target.test)
            .map(|target| format!(":__bsmr_test_test_{}", target.name))
            .collect();
        for target in &self.targets {
            let Some(entrypoint) = target.entrypoint(&self.name)? else {
                continue;
            };
            let kind = entrypoint.kind();
            let name = if kind == "lib" { "lib" } else { &target.name };
            let test = format!("__bsmr_test_{kind}_{}", target.name);
            let mut tests = integration_tests.clone();
            if target.test {
                tests.push(format!(":{test}"));
            }
            for (mode, alias) in [(Mode::Build, name), (Mode::Test, test.as_str())] {
                if (mode == Mode::Test && !target.test) || (mode == Mode::Build && kind == "test") {
                    continue;
                }
                let directory = if path.is_empty() {
                    String::new()
                } else {
                    format!("{path}/")
                };
                let actual = format!("{cell}//{directory}{}:root", entrypoint.child_name(mode));
                source.push_str(&format!(
                    "alias(name = {}, actual = {}, tests = {}, visibility = [\"PUBLIC\"])\n",
                    json(alias)?,
                    json(&actual)?,
                    json(&if mode == Mode::Build {
                        tests.clone()
                    } else {
                        Vec::new()
                    })?
                ));
            }
            if kind != "test" {
                names.push((name, tests));
            }
        }
        if let [(actual, tests)] = names.as_slice() {
            let alias = Path::new(path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(&self.name);
            if alias != *actual {
                source.push_str(&format!(
                    "alias(name = {}, actual = {}, tests = {}, visibility = [\"PUBLIC\"])\n",
                    json(alias)?,
                    json(&format!(":{actual}"))?,
                    json(tests)?
                ));
            }
        }
        Ok(source)
    }
}

/// Resolve invocation patterns against Cargo metadata without evaluating generated rules.
pub(super) fn selected(
    bytes: &[u8],
    root: &Path,
    cell: CellName,
    mode: Mode,
    patterns: &[ParsedPattern<ConfiguredProvidersPatternExtra>],
) -> Result<Vec<Entry>, RustGraphError> {
    let metadata: Metadata = serde_json::from_slice(bytes)?;
    let mut entries = Vec::new();
    for package in &metadata.packages {
        let path = package
            .manifest_path
            .parent()
            .expect("manifest has a parent")
            .strip_prefix(root)
            .map_err(|_| RustGraphError::Outside(package.manifest_path.clone()))?;
        let path = path.to_string_lossy().replace('\\', "/");
        let label = PackageLabel::new(
            cell,
            &CellRelativePathBuf::try_from(path.clone())
                .map_err(|_| unsupported(&package.name, "invalid package path"))?,
        )
        .map_err(|_| unsupported(&package.name, "invalid package label"))?;
        entries.extend(package.selected(label, &path, mode, patterns)?);
    }
    entries.sort_by_key(|entry| {
        (
            entry.package.to_string(),
            entry.target.kind().to_owned(),
            entry.target.name().to_owned(),
        )
    });
    entries.dedup();
    Ok(entries)
}

impl Package {
    /// Apply public aliases to metadata targets without resolving dependencies.
    fn selected(
        &self,
        label: PackageLabel,
        path: &str,
        mode: Mode,
        patterns: &[ParsedPattern<ConfiguredProvidersPatternExtra>],
    ) -> Result<Vec<Entry>, RustGraphError> {
        let mut entries = Vec::new();
        let mut normal = Vec::new();
        for target in &self.targets {
            if let Some(entry) = target.entrypoint(&self.name)? {
                if entry.kind() != "test" {
                    normal.push(if entry.kind() == "lib" {
                        "lib".to_owned()
                    } else {
                        target.name.clone()
                    });
                }
            }
        }
        let package_alias = (normal.len() == 1).then(|| {
            Path::new(&path)
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or(&self.name)
                .to_owned()
        });
        for target in &self.targets {
            let Some(entrypoint) = target.entrypoint(&self.name)? else {
                continue;
            };
            if (mode == Mode::Test && !target.test)
                || (mode == Mode::Build && entrypoint.kind() == "test")
            {
                continue;
            }
            let alias = if entrypoint.kind() == "lib" {
                "lib"
            } else {
                &target.name
            };
            let test = format!("__bsmr_test_{}_{}", entrypoint.kind(), target.name);
            let matches = patterns.iter().any(|pattern| match pattern {
                ParsedPattern::Target(owner, name, _) => {
                    *owner == label
                        && (name.as_str() == alias
                            || (mode == Mode::Test && name.as_str() == test)
                            || (mode == Mode::Test
                                && entrypoint.kind() == "test"
                                && normal.iter().any(|alias| alias == name.as_str()))
                            || package_alias
                                .as_ref()
                                .is_some_and(|alias| alias == name.as_str()))
                }
                ParsedPattern::Package(owner) => *owner == label,
                ParsedPattern::Recursive(prefix) => {
                    label.as_cell_path().starts_with(prefix.as_ref())
                }
            });
            if matches {
                entries.push(Entry {
                    package: label,
                    mode,
                    target: entrypoint,
                });
            }
        }
        Ok(entries)
    }
}

/// Cargo dependencies own source inputs even when they expose no workspace entrypoints.
pub(super) fn sources() -> &'static str {
    "filegroup(name = \"__bsmr_sources\", srcs = glob([\"**\"], exclude = [\"BUILD.bsmr\", \"target/**\", \"bsmr-out/**\", \".git/**\"]), copy = True, has_content_based_path = True, visibility = [\"PUBLIC\"])\n"
}

/// Resolve a descriptor against Cargo's target catalog before planning its dependencies.
pub fn package_name(bytes: &[u8], root: &Path, entry: &Entry) -> Result<String, RustGraphError> {
    let metadata: Metadata = serde_json::from_slice(bytes)?;
    let manifest = root
        .join(entry.package.as_cell_path().path().as_str())
        .join("Cargo.toml");
    let package = metadata
        .packages
        .iter()
        .find(|package| package.manifest_path == manifest)
        .ok_or_else(|| {
            unsupported(
                &entry.package.to_string(),
                "package absent from Cargo catalog",
            )
        })?;
    let present = package.targets.iter().any(|target| {
        target.name == entry.target.name()
            && (target.kind == [entry.target.kind()]
                || (entry.target.kind() == "lib" && libraries::is_library(&target.kind)))
            && (entry.mode == Mode::Build || target.test)
    });
    if !present {
        return Err(unsupported(
            &entry.package.to_string(),
            "target absent from Cargo catalog",
        ));
    }
    Ok(package.name.clone())
}

impl CargoTarget {
    /// Normalize ordinary library kinds and reject names reserved by the importer.
    fn entrypoint(&self, package: &str) -> Result<Option<Target>, RustGraphError> {
        let target = match self.kind.as_slice() {
            kinds if libraries::is_library(kinds) => Target::Lib(self.name.clone()),
            [kind] if kind == "bin" => Target::Bin(self.name.clone()),
            [kind] if kind == "test" => Target::Test(self.name.clone()),
            _ => return Ok(None),
        };
        if self.name.starts_with("__bsmr_") {
            return Err(unsupported(
                package,
                "target names beginning with `__bsmr_` are reserved",
            ));
        }
        if target.kind() == "bin" && self.name == "lib" {
            return Err(unsupported(package, "binary target name `lib` is reserved"));
        }
        Ok(Some(target))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_preserves_target_names_without_planning_dependencies() {
        let metadata = serde_json::json!({"version":1,"workspace_root":"/workspace","packages":[{"name":"same","manifest_path":"/workspace/app/Cargo.toml","targets":[{"name":"same","src_path":"/workspace/app/source.rs","kind":["rlib"],"test":true},{"name":"same","src_path":"/workspace/app/source.rs","kind":["bin"],"test":true},{"name":"build-script-build","src_path":"/workspace/app/source.rs","kind":["custom-build"],"test":false}]}]});
        let rules = render(
            &serde_json::to_vec(&metadata).unwrap(),
            Path::new("/workspace"),
            "root",
        )
        .unwrap();
        let app = &rules["app"];
        assert!(app.contains("__bsmr_cargo_build_lib_same"));
        assert!(app.contains("__bsmr_cargo_build_bin_same"));
        assert!(app.contains("__bsmr_test_lib_same"));
        assert!(app.contains("__bsmr_test_bin_same"));
        assert!(!app.contains("custom-build"));
    }

    #[test]
    fn integration_names_do_not_replace_build_targets() {
        let metadata = serde_json::json!({"version":1,"workspace_root":"/workspace","packages":[{"name":"app","manifest_path":"/workspace/app/Cargo.toml","targets":[{"name":"app","src_path":"/workspace/app/source.rs","kind":["bin"],"test":false},{"name":"app","src_path":"/workspace/app/source.rs","kind":["test"],"test":true}]}]});
        let rules = render(
            &serde_json::to_vec(&metadata).unwrap(),
            Path::new("/workspace"),
            "root",
        )
        .unwrap();
        let app = &rules["app"];
        assert!(app.contains("__bsmr_cargo_test_test_app"));
        assert!(!app.contains("__bsmr_cargo_build_test_app"));
        assert!(app.contains("tests = [\":__bsmr_test_test_app\"]"));
        assert_eq!(app.matches("alias(name = \"app\"").count(), 1);
    }
}
