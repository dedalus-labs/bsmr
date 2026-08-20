//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Normalizes pnpm workspace manifests into a deterministic directed graph.

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use bsmr_core::cells::paths::CellRelativePathBuf;
use serde::Deserialize;

mod dice;
mod lockfile;
mod manifest;
mod native_build;
mod toolchain;

pub use dice::HasPnpmWorkspaceGraph;
use lockfile::PnpmLock;
use manifest::PnpmWorkspace;
pub use native_build::NativeTypeScriptBuildError;
pub use native_build::is_native_pnpm_workspace;
pub use native_build::render_typescript_build_file;

/// Failure to parse one workspace `package.json`.
#[derive(Debug, bsmr_error::Error)]
#[bsmr(tag = Input)]
enum PackageManifestError {
    /// The manifest is not valid JSON or has a dependency with a non-string specifier.
    #[error("invalid package manifest at `{root}`")]
    InvalidJson {
        root: CellRelativePathBuf,
        #[source]
        source: serde_json::Error,
    },
    /// A stable package name is required for graph identity.
    #[error("package manifest at `{0}` must declare a non-empty `name`")]
    MissingName(CellRelativePathBuf),
}

/// The manifest section that contributed an internal build edge.
#[derive(
    Clone,
    Copy,
    Debug,
    Eq,
    Ord,
    PartialEq,
    PartialOrd,
    allocative::Allocative,
    pagable::Pagable
)]
enum DependencySection {
    /// Runtime dependency.
    Dependency,
    /// Development-only dependency needed to build or test the package.
    DevDependency,
    /// Optional runtime dependency.
    OptionalDependency,
    /// Peer dependency supplied by the consuming package.
    PeerDependency,
}

#[derive(Clone, Debug, Eq, PartialEq, allocative::Allocative, pagable::Pagable)]
struct DependencyDeclaration {
    section: DependencySection,
    specifier: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PackageJson {
    name: Option<String>,
    engines: Option<PackageEngines>,
    package_manager: Option<String>,
    #[serde(default)]
    dependencies: BTreeMap<String, String>,
    #[serde(default)]
    dev_dependencies: BTreeMap<String, String>,
    #[serde(default)]
    optional_dependencies: BTreeMap<String, String>,
    #[serde(default)]
    peer_dependencies: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize)]
struct PackageEngines {
    node: Option<String>,
}

/// One parsed package manifest and its cell-relative workspace root.
#[derive(Clone, Debug, Eq, PartialEq, allocative::Allocative, pagable::Pagable)]
struct WorkspacePackage {
    root: CellRelativePathBuf,
    name: String,
    node_requirement: Option<String>,
    package_manager: Option<String>,
    dependencies: BTreeMap<String, Vec<DependencyDeclaration>>,
}

impl WorkspacePackage {
    /// Parses a package manifest while preserving dependency-section provenance.
    fn parse(root: CellRelativePathBuf, source: &str) -> Result<Self, PackageManifestError> {
        let manifest: PackageJson =
            serde_json::from_str(source).map_err(|source| PackageManifestError::InvalidJson {
                root: root.clone(),
                source,
            })?;
        let Some(name) = manifest.name.filter(|name| !name.is_empty()) else {
            return Err(PackageManifestError::MissingName(root));
        };
        let mut dependencies = BTreeMap::new();
        extend_dependencies(
            &mut dependencies,
            DependencySection::Dependency,
            manifest.dependencies,
        );
        extend_dependencies(
            &mut dependencies,
            DependencySection::DevDependency,
            manifest.dev_dependencies,
        );
        extend_dependencies(
            &mut dependencies,
            DependencySection::OptionalDependency,
            manifest.optional_dependencies,
        );
        extend_dependencies(
            &mut dependencies,
            DependencySection::PeerDependency,
            manifest.peer_dependencies,
        );
        Ok(Self {
            root,
            name,
            node_requirement: manifest.engines.and_then(|engines| engines.node),
            package_manager: manifest.package_manager,
            dependencies,
        })
    }
}

/// Adds one manifest section to the package's dependency declarations.
fn extend_dependencies(
    dependencies: &mut BTreeMap<String, Vec<DependencyDeclaration>>,
    section: DependencySection,
    declarations: BTreeMap<String, String>,
) {
    for (name, specifier) in declarations {
        dependencies
            .entry(name)
            .or_default()
            .push(DependencyDeclaration { section, specifier });
    }
}

/// A package dependency graph cannot be normalized without resolving this invariant.
#[derive(Clone, Debug, Eq, PartialEq, bsmr_error::Error)]
#[bsmr(tag = Input)]
enum WorkspaceGraphError {
    /// Two roots cannot own the same npm package identity.
    #[error("workspace package `{name}` is declared by both `{first_root}` and `{second_root}`")]
    DuplicatePackageName {
        name: String,
        first_root: CellRelativePathBuf,
        second_root: CellRelativePathBuf,
    },
    /// The workspace protocol never resolves through the registry.
    #[error("workspace package `{package}` depends on missing workspace package `{dependency}`")]
    MissingWorkspaceDependency { package: String, dependency: String },
    /// A same-name semver dependency depends on pnpm configuration or lockfile state.
    #[error(
        "workspace package `{package}` declares same-name package `{dependency}` as `{specifier}`; package.json alone cannot distinguish the registry from the workspace"
    )]
    AmbiguousLocalDependency {
        package: String,
        dependency: String,
        specifier: String,
    },
    /// Alias and relative workspace specifiers require lockfile-aware resolution.
    #[error(
        "workspace package `{package}` uses unsupported workspace specifier `{specifier}` for `{dependency}`"
    )]
    UnsupportedWorkspaceSpecifier {
        package: String,
        dependency: String,
        specifier: String,
    },
    /// Remaining packages could not be topologically ordered.
    #[error("workspace dependency cycle blocks packages: {packages:?}")]
    DependencyCycle { packages: Vec<String> },
}

/// One normalized package node in the workspace graph.
#[derive(Clone, Debug, Eq, PartialEq, allocative::Allocative, pagable::Pagable)]
struct WorkspaceProject {
    root: CellRelativePathBuf,
    dependencies: BTreeMap<String, WorkspaceDependency>,
}

/// One internal edge with its exact manifest declarations.
#[derive(Clone, Debug, Eq, PartialEq, allocative::Allocative, pagable::Pagable)]
struct WorkspaceDependency {
    declarations: BTreeMap<DependencySection, String>,
}

impl WorkspaceProject {
    /// Returns the package's normalized cell-relative root.
    #[must_use]
    fn root(&self) -> &CellRelativePathBuf {
        &self.root
    }

    /// Returns internal dependencies in canonical npm-name order.
    fn dependencies(&self) -> impl ExactSizeIterator<Item = &str> {
        self.dependencies.keys().map(String::as_str)
    }
}

/// A deterministic directed acyclic graph of pnpm workspace packages.
#[derive(Clone, Debug, Eq, PartialEq, allocative::Allocative, pagable::Pagable)]
pub struct WorkspaceGraph {
    packages: BTreeMap<String, WorkspaceProject>,
    node_toolchain: Option<NodeWorkspaceToolchain>,
}

/// Native Node requirements read from the root package and workspace manifests.
#[derive(Clone, Debug, Eq, PartialEq, allocative::Allocative, pagable::Pagable)]
struct NodeWorkspaceToolchain {
    node_requirement: String,
    package_manager: String,
    runtime_version: Option<String>,
}

impl NodeWorkspaceToolchain {
    /// Returns the npm-compatible Node version requirement.
    #[must_use]
    fn node_requirement(&self) -> &str {
        &self.node_requirement
    }

    /// Returns the exact Corepack-style package-manager identity.
    #[must_use]
    fn package_manager(&self) -> &str {
        &self.package_manager
    }

    /// Returns pnpm's optional exact project runtime pin.
    #[must_use]
    fn runtime_version(&self) -> Option<&str> {
        self.runtime_version.as_deref()
    }
}

impl WorkspaceGraph {
    /// Resolves explicit workspace edges and rejects ambiguous or cyclic graphs.
    #[cfg(test)]
    fn build(
        packages: impl IntoIterator<Item = WorkspacePackage>,
    ) -> Result<Self, WorkspaceGraphError> {
        Self::build_with_lock(packages, None, None)
    }

    /// Resolves one graph using the frozen lockfile for ambiguous semver edges.
    fn build_with_lock(
        packages: impl IntoIterator<Item = WorkspacePackage>,
        workspace: Option<&PnpmWorkspace>,
        lock: Option<&PnpmLock>,
    ) -> Result<Self, WorkspaceGraphError> {
        let packages = index_packages(packages)?;
        let node_toolchain = packages
            .values()
            .find(|package| package.root.is_empty())
            .and_then(|package| {
                package
                    .node_requirement
                    .clone()
                    .zip(package.package_manager.clone())
            })
            .map(
                |(node_requirement, package_manager)| NodeWorkspaceToolchain {
                    node_requirement,
                    package_manager,
                    runtime_version: workspace
                        .and_then(PnpmWorkspace::use_node_version)
                        .map(str::to_owned),
                },
            );
        let projects = resolve_projects(&packages, lock)?;
        ensure_acyclic(&projects)?;
        Ok(Self {
            packages: projects,
            node_toolchain,
        })
    }

    /// Returns one package node by its npm name.
    #[must_use]
    fn package(&self, name: &str) -> Option<&WorkspaceProject> {
        self.packages.get(name)
    }

    /// Returns the package name owning one normalized workspace root.
    fn package_name_at_root(&self, root: &CellRelativePathBuf) -> Option<&str> {
        self.packages
            .iter()
            .find(|(_, project)| &project.root == root)
            .map(|(name, _)| name.as_str())
    }

    /// Returns the root manifest's exact native Node toolchain requirements.
    #[must_use]
    fn node_toolchain(&self) -> Option<&NodeWorkspaceToolchain> {
        self.node_toolchain.as_ref()
    }
}

/// Indexes packages by npm name while retaining both roots in duplicate diagnostics.
fn index_packages(
    packages: impl IntoIterator<Item = WorkspacePackage>,
) -> Result<BTreeMap<String, WorkspacePackage>, WorkspaceGraphError> {
    let mut indexed: BTreeMap<String, WorkspacePackage> = BTreeMap::new();
    for package in packages {
        if let Some(first) = indexed.get(&package.name) {
            return Err(WorkspaceGraphError::DuplicatePackageName {
                name: package.name,
                first_root: first.root.clone(),
                second_root: package.root,
            });
        }
        indexed.insert(package.name.clone(), package);
    }
    Ok(indexed)
}

/// Resolves workspace-protocol declarations into typed internal edges.
fn resolve_projects(
    packages: &BTreeMap<String, WorkspacePackage>,
    lock: Option<&PnpmLock>,
) -> Result<BTreeMap<String, WorkspaceProject>, WorkspaceGraphError> {
    packages
        .iter()
        .map(|(name, package)| {
            let dependencies = resolve_dependencies(package, packages, lock)?;
            Ok((
                name.clone(),
                WorkspaceProject {
                    root: package.root.clone(),
                    dependencies,
                },
            ))
        })
        .collect()
}

/// Resolves one package's declarations without consulting ambient pnpm settings.
fn resolve_dependencies(
    package: &WorkspacePackage,
    packages: &BTreeMap<String, WorkspacePackage>,
    lock: Option<&PnpmLock>,
) -> Result<BTreeMap<String, WorkspaceDependency>, WorkspaceGraphError> {
    let mut resolved = BTreeMap::new();
    for (dependency, declarations) in &package.dependencies {
        for declaration in declarations {
            let Some(range) = declaration.specifier.strip_prefix("workspace:") else {
                if packages.contains_key(dependency) {
                    if lock.is_some_and(|lock| {
                        lock.resolves_to_workspace(
                            &package.root,
                            declaration.section,
                            dependency,
                            &declaration.specifier,
                            &packages[dependency].root,
                        )
                    }) {
                        resolved
                            .entry(dependency.clone())
                            .or_insert_with(|| WorkspaceDependency {
                                declarations: BTreeMap::new(),
                            })
                            .declarations
                            .insert(declaration.section, declaration.specifier.clone());
                        continue;
                    }
                    if lock.is_some_and(|lock| {
                        lock.resolves_to_registry(
                            &package.root,
                            declaration.section,
                            dependency,
                            &declaration.specifier,
                        )
                    }) {
                        continue;
                    }
                    return Err(WorkspaceGraphError::AmbiguousLocalDependency {
                        package: package.name.clone(),
                        dependency: dependency.clone(),
                        specifier: declaration.specifier.clone(),
                    });
                }
                continue;
            };
            if !is_direct_workspace_range(range) {
                return Err(WorkspaceGraphError::UnsupportedWorkspaceSpecifier {
                    package: package.name.clone(),
                    dependency: dependency.clone(),
                    specifier: declaration.specifier.clone(),
                });
            }
            if !packages.contains_key(dependency) {
                return Err(WorkspaceGraphError::MissingWorkspaceDependency {
                    package: package.name.clone(),
                    dependency: dependency.clone(),
                });
            }
            resolved
                .entry(dependency.clone())
                .or_insert_with(|| WorkspaceDependency {
                    declarations: BTreeMap::new(),
                })
                .declarations
                .insert(declaration.section, declaration.specifier.clone());
        }
    }
    Ok(resolved)
}

/// Recognizes workspace ranges whose target is exactly the dependency key.
fn is_direct_workspace_range(range: &str) -> bool {
    matches!(
        range.as_bytes().first(),
        Some(b'*' | b'^' | b'~' | b'<' | b'>' | b'=' | b'0'..=b'9')
    )
}

/// Uses deterministic leaf-first Kahn ordering to reject cyclic dependency graphs.
fn ensure_acyclic(
    projects: &BTreeMap<String, WorkspaceProject>,
) -> Result<(), WorkspaceGraphError> {
    let mut dependency_counts = projects
        .iter()
        .map(|(name, project)| (name.clone(), project.dependencies.len()))
        .collect::<BTreeMap<_, _>>();
    let mut dependents = BTreeMap::<String, Vec<String>>::new();
    for (name, project) in projects {
        for dependency in project.dependencies.keys() {
            dependents
                .entry(dependency.clone())
                .or_default()
                .push(name.clone());
        }
    }
    let mut ready = dependency_counts
        .iter()
        .filter(|(_, count)| **count == 0)
        .map(|(name, _)| name.clone())
        .collect::<BTreeSet<_>>();
    while let Some(name) = ready.pop_first() {
        for dependent in dependents.get(&name).into_iter().flatten() {
            let count = dependency_counts
                .get_mut(dependent)
                .expect("indexed dependent");
            *count -= 1;
            if *count == 0 {
                ready.insert(dependent.clone());
            }
        }
    }
    let packages = dependency_counts
        .into_iter()
        .filter(|(_, count)| *count != 0)
        .map(|(name, _)| name)
        .collect::<Vec<_>>();
    if packages.is_empty() {
        Ok(())
    } else {
        Err(WorkspaceGraphError::DependencyCycle { packages })
    }
}

#[cfg(test)]
mod tests {
    use bsmr_core::cells::paths::CellRelativePathBuf;

    use super::DependencySection;
    use super::PnpmLock;
    use super::PnpmWorkspace;
    use super::WorkspaceGraph;
    use super::WorkspaceGraphError;
    use super::WorkspacePackage;

    /// Parses one package fixture rooted at a normalized workspace path.
    fn package(root: &str, manifest: &str) -> WorkspacePackage {
        WorkspacePackage::parse(
            CellRelativePathBuf::try_from(root.to_owned()).unwrap(),
            manifest,
        )
        .unwrap()
    }

    #[test]
    fn invariant_workspace_dependencies_are_directional_and_deterministic() {
        let app = package(
            "apps/api",
            r#"{
                "name": "@acme/api",
                "dependencies": {
                    "zod": "^4.0.0",
                    "@acme/core": "workspace:^"
                },
                "devDependencies": {
                    "@acme/testkit": "workspace:*"
                }
            }"#,
        );
        let core = package("packages/core", r#"{"name":"@acme/core"}"#);
        let testkit = package("packages/testkit", r#"{"name":"@acme/testkit"}"#);

        let graph = WorkspaceGraph::build([testkit, app, core]).unwrap();

        assert_eq!(
            graph
                .packages
                .keys()
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["@acme/api", "@acme/core", "@acme/testkit"]
        );
        assert_eq!(
            graph
                .package("@acme/api")
                .unwrap()
                .dependencies()
                .collect::<Vec<_>>(),
            ["@acme/core", "@acme/testkit"]
        );
        assert_eq!(
            graph.package("@acme/core").unwrap().dependencies().count(),
            0
        );
        let sections = graph
            .package("@acme/api")
            .unwrap()
            .dependencies
            .get("@acme/testkit")
            .unwrap();
        assert_eq!(sections.declarations.len(), 1);
        assert_eq!(
            sections
                .declarations
                .get(&DependencySection::DevDependency)
                .map(String::as_str),
            Some("workspace:*")
        );
    }

    #[test]
    fn invariant_root_manifest_owns_the_exact_node_toolchain_contract() {
        let graph = WorkspaceGraph::build([package(
            "",
            r#"{
                "name": "@acme/root",
                "engines": {"node": ">=24.0.0"},
                "packageManager": "pnpm@10.30.3+sha512.c961d1e0a2d8e354ecaa5166b822516668b7f44cb5bd95122d590dd81922f606f5473b6d23ec4a5be05e7fcd18e8488d47d978bbe981872f1145d06e9a740017"
            }"#,
        )])
        .unwrap();

        let toolchain = graph.node_toolchain().unwrap();

        assert_eq!(toolchain.node_requirement(), ">=24.0.0");
        assert_eq!(
            toolchain.package_manager(),
            "pnpm@10.30.3+sha512.c961d1e0a2d8e354ecaa5166b822516668b7f44cb5bd95122d590dd81922f606f5473b6d23ec4a5be05e7fcd18e8488d47d978bbe981872f1145d06e9a740017"
        );
    }

    #[test]
    fn invariant_duplicate_package_names_are_rejected() {
        let first = package("packages/first", r#"{"name":"@acme/core"}"#);
        let second = package("packages/second", r#"{"name":"@acme/core"}"#);

        let error = WorkspaceGraph::build([first, second]).unwrap_err();

        assert!(matches!(
            error,
            WorkspaceGraphError::DuplicatePackageName { .. }
        ));
    }

    #[test]
    fn invariant_missing_workspace_dependencies_are_rejected() {
        let app = package(
            "apps/api",
            r#"{
                "name": "@acme/api",
                "dependencies": {"@acme/missing": "workspace:*"}
            }"#,
        );

        let error = WorkspaceGraph::build([app]).unwrap_err();

        assert!(matches!(
            error,
            WorkspaceGraphError::MissingWorkspaceDependency { .. }
        ));
    }

    #[test]
    fn invariant_workspace_dependency_cycles_are_rejected() {
        let first = package(
            "packages/first",
            r#"{
                "name": "@acme/first",
                "dependencies": {"@acme/second": "workspace:*"}
            }"#,
        );
        let second = package(
            "packages/second",
            r#"{
                "name": "@acme/second",
                "dependencies": {"@acme/first": "workspace:*"}
            }"#,
        );

        let error = WorkspaceGraph::build([first, second]).unwrap_err();

        assert_eq!(
            error,
            WorkspaceGraphError::DependencyCycle {
                packages: vec!["@acme/first".to_owned(), "@acme/second".to_owned()],
            }
        );
    }

    #[test]
    fn invariant_local_semver_dependencies_are_not_guessed() {
        let app = package(
            "apps/api",
            r#"{
                "name": "@acme/api",
                "dependencies": {"@acme/core": "^1.0.0"}
            }"#,
        );
        let core = package("packages/core", r#"{"name":"@acme/core"}"#);

        let error = WorkspaceGraph::build([app, core]).unwrap_err();

        assert!(matches!(
            error,
            WorkspaceGraphError::AmbiguousLocalDependency { .. }
        ));
    }

    #[test]
    fn invariant_lockfile_distinguishes_registry_and_workspace_semver_dependencies() {
        let app = package(
            "apps/api",
            r#"{
                "name": "@acme/api",
                "dependencies": {
                    "@acme/core": "^1.0.0",
                    "@acme/registry": "^1.0.0"
                }
            }"#,
        );
        let core = package("packages/core", r#"{"name":"@acme/core"}"#);
        let registry = package("packages/registry", r#"{"name":"@acme/registry"}"#);
        let lock = PnpmLock::parse(
            r#"
lockfileVersion: '9.0'
importers:
  apps/api:
    dependencies:
      '@acme/core':
        specifier: ^1.0.0
        version: link:../../packages/core
      '@acme/registry':
        specifier: ^1.0.0
        version: 1.2.3
"#,
        )
        .unwrap();

        let graph =
            WorkspaceGraph::build_with_lock([app, core, registry], None, Some(&lock)).unwrap();

        assert_eq!(
            graph
                .package("@acme/api")
                .unwrap()
                .dependencies()
                .collect::<Vec<_>>(),
            ["@acme/core"]
        );
    }

    #[test]
    fn invariant_workspace_aliases_wait_for_lockfile_resolution() {
        let app = package(
            "apps/api",
            r#"{
                "name": "@acme/api",
                "dependencies": {"core-alias": "workspace:@acme/core@*"}
            }"#,
        );
        let core = package("packages/core", r#"{"name":"@acme/core"}"#);

        let error = WorkspaceGraph::build([app, core]).unwrap_err();

        assert!(matches!(
            error,
            WorkspaceGraphError::UnsupportedWorkspaceSpecifier { .. }
        ));
    }

    #[test]
    fn invariant_workspace_manifest_supports_explicit_and_root_only_workspaces() {
        let workspace = PnpmWorkspace::parse(
            r#"
packages:
  - "apps/*"
  - packages/typescript/**
"#,
        )
        .unwrap();

        assert_eq!(
            workspace
                .select_package_roots(
                    [
                        "apps/api",
                        "packages/typescript/core",
                        "packages/python/core"
                    ]
                    .map(|path| CellRelativePathBuf::try_from(path.to_owned()).unwrap())
                )
                .unwrap()
                .iter()
                .map(|path| path.as_str())
                .collect::<Vec<_>>(),
            ["apps/api", "packages/typescript/core"]
        );
        let root_only = PnpmWorkspace::parse("allowBuilds:\n  esbuild: false").unwrap();
        assert_eq!(
            root_only
                .select_package_roots(
                    ["packages/core", ""]
                        .map(|path| CellRelativePathBuf::try_from(path.to_owned()).unwrap())
                )
                .unwrap()
                .iter()
                .map(|path| path.as_str())
                .collect::<Vec<_>>(),
            [""]
        );
        assert_eq!(
            PnpmWorkspace::parse("packages: []")
                .unwrap()
                .select_package_roots(
                    ["", "apps/api"]
                        .map(|path| CellRelativePathBuf::try_from(path.to_owned()).unwrap())
                )
                .unwrap()
                .iter()
                .map(|path| path.as_str())
                .collect::<Vec<_>>(),
            [""]
        );
        assert!(PnpmWorkspace::parse("packages: packages/*").is_err());
        assert!(PnpmWorkspace::parse("useNodeVersion: []").is_err());
    }

    #[test]
    fn invariant_workspace_selection_honors_globs_exclusions_and_root() {
        let workspace = PnpmWorkspace::parse(
            r#"
packages:
  - apps/*
  - packages/typescript/**
  - "!**/fixtures/**"
"#,
        )
        .unwrap();
        let candidates = [
            "packages/typescript/zeta",
            "apps/api",
            "apps/api/nested",
            "packages/typescript/fixtures/rejected",
            "",
            "crates/rust",
        ]
        .map(|path| CellRelativePathBuf::try_from(path.to_owned()).unwrap());

        let selected = workspace.select_package_roots(candidates).unwrap();

        assert_eq!(
            selected
                .iter()
                .map(|path| path.as_str())
                .collect::<Vec<_>>(),
            ["", "apps/api", "packages/typescript/zeta"]
        );
    }
}
