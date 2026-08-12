//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Normalizes pnpm workspace manifests into a deterministic directed graph.

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use bsmr_core::cells::name::CellName;
use bsmr_core::cells::paths::CellRelativePathBuf;
use bsmr_core::package::PackageLabel;
use bsmr_core::target::label::label::TargetLabel;
use bsmr_core::target::name::TargetName;
use bsmr_error::internal_error;
use serde::Deserialize;

mod dice;
mod manifest;
mod native_build;
mod toolchain;

pub use dice::HasPnpmWorkspaceGraph;
pub use manifest::PnpmWorkspace;
pub use manifest::PnpmWorkspaceError;
pub use native_build::NativeTypeScriptBuildError;
pub use native_build::render_typescript_build_file;

/// Failure to parse one workspace `package.json`.
#[derive(Debug, bsmr_error::Error)]
#[bsmr(tag = Input)]
pub enum PackageManifestError {
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
pub enum DependencySection {
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
pub struct WorkspacePackage {
    root: CellRelativePathBuf,
    name: String,
    node_requirement: Option<String>,
    package_manager: Option<String>,
    dependencies: BTreeMap<String, Vec<DependencyDeclaration>>,
}

impl WorkspacePackage {
    /// Parses a package manifest while preserving dependency-section provenance.
    pub fn parse(root: CellRelativePathBuf, source: &str) -> Result<Self, PackageManifestError> {
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

    /// Returns the package's stable npm name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the package's normalized cell-relative root.
    #[must_use]
    pub fn root(&self) -> &CellRelativePathBuf {
        &self.root
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
pub enum WorkspaceGraphError {
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
pub struct WorkspaceProject {
    root: CellRelativePathBuf,
    dependencies: BTreeMap<String, WorkspaceDependency>,
}

/// One internal edge with its exact manifest declarations.
#[derive(Clone, Debug, Eq, PartialEq, allocative::Allocative, pagable::Pagable)]
pub struct WorkspaceDependency {
    declarations: BTreeMap<DependencySection, String>,
}

impl WorkspaceDependency {
    /// Returns contributing manifest sections in canonical order.
    pub fn sections(&self) -> impl ExactSizeIterator<Item = DependencySection> + '_ {
        self.declarations.keys().copied()
    }

    /// Returns the exact workspace protocol specifier from one manifest section.
    #[must_use]
    pub fn specifier(&self, section: DependencySection) -> Option<&str> {
        self.declarations.get(&section).map(String::as_str)
    }
}

impl WorkspaceProject {
    /// Returns the package's normalized cell-relative root.
    #[must_use]
    pub fn root(&self) -> &CellRelativePathBuf {
        &self.root
    }

    /// Returns internal dependencies in canonical npm-name order.
    pub fn dependencies(&self) -> impl ExactSizeIterator<Item = &str> {
        self.dependencies.keys().map(String::as_str)
    }

    /// Returns one internal edge and its source declarations.
    #[must_use]
    pub fn dependency(&self, dependency: &str) -> Option<&WorkspaceDependency> {
        self.dependencies.get(dependency)
    }
}

/// A deterministic directed acyclic graph of pnpm workspace packages.
#[derive(Clone, Debug, Eq, PartialEq, allocative::Allocative, pagable::Pagable)]
pub struct WorkspaceGraph {
    packages: BTreeMap<String, WorkspaceProject>,
    node_toolchain: Option<NodeWorkspaceToolchain>,
}

/// Exact native Node workspace requirements read from the root package manifest.
#[derive(Clone, Debug, Eq, PartialEq, allocative::Allocative, pagable::Pagable)]
pub struct NodeWorkspaceToolchain {
    node_requirement: String,
    package_manager: String,
}

impl NodeWorkspaceToolchain {
    /// Returns the npm-compatible Node version requirement.
    #[must_use]
    pub fn node_requirement(&self) -> &str {
        &self.node_requirement
    }

    /// Returns the exact Corepack-style package-manager identity.
    #[must_use]
    pub fn package_manager(&self) -> &str {
        &self.package_manager
    }
}

/// A conventional target inferred for every TypeScript workspace project.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum WorkspaceTargetKind {
    /// JavaScript and declaration emission.
    Library,
    /// Semantic checking without JavaScript emission.
    Typecheck,
}

impl WorkspaceTargetKind {
    /// Returns the conventional target name for this capability.
    fn target_name(self) -> &'static str {
        match self {
            Self::Library => "lib",
            Self::Typecheck => "typecheck",
        }
    }

    /// Returns the stable slot used by the compact per-project label table.
    fn index(self) -> usize {
        match self {
            Self::Library => 0,
            Self::Typecheck => 1,
        }
    }
}

/// One native BSMR target plus its directed workspace dependencies.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorkspaceTarget {
    kind: WorkspaceTargetKind,
    label: TargetLabel,
    dependencies: Vec<TargetLabel>,
}

impl WorkspaceTarget {
    /// Returns the capability implemented by this target.
    #[must_use]
    pub fn kind(&self) -> WorkspaceTargetKind {
        self.kind
    }

    /// Returns the canonical unconfigured BSMR target label.
    #[must_use]
    pub fn label(&self) -> &TargetLabel {
        &self.label
    }

    /// Returns dependency labels in canonical npm-name order.
    pub fn dependencies(&self) -> impl ExactSizeIterator<Item = &TargetLabel> {
        self.dependencies.iter()
    }
}

impl WorkspaceGraph {
    /// Resolves explicit workspace edges and rejects ambiguous or cyclic graphs.
    pub fn build(
        packages: impl IntoIterator<Item = WorkspacePackage>,
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
                },
            );
        let projects = resolve_projects(&packages)?;
        ensure_acyclic(&projects)?;
        Ok(Self {
            packages: projects,
            node_toolchain,
        })
    }

    /// Returns package names in canonical lexical order.
    pub fn package_names(&self) -> impl ExactSizeIterator<Item = &str> {
        self.packages.keys().map(String::as_str)
    }

    /// Returns one package node by its npm name.
    #[must_use]
    pub fn package(&self, name: &str) -> Option<&WorkspaceProject> {
        self.packages.get(name)
    }

    /// Returns one package's internal dependencies in canonical lexical order.
    pub fn dependencies(&self, name: &str) -> Option<impl ExactSizeIterator<Item = &str>> {
        self.package(name).map(WorkspaceProject::dependencies)
    }

    /// Returns the package name owning one normalized workspace root.
    pub fn package_name_at_root(&self, root: &CellRelativePathBuf) -> Option<&str> {
        self.packages
            .iter()
            .find(|(_, project)| &project.root == root)
            .map(|(name, _)| name.as_str())
    }

    /// Returns the root manifest's exact native Node toolchain requirements.
    #[must_use]
    pub fn node_toolchain(&self) -> Option<&NodeWorkspaceToolchain> {
        self.node_toolchain.as_ref()
    }

    /// Lowers package roots and edges into BSMR's native target-label IR.
    pub fn lower(&self, cell: CellName) -> bsmr_error::Result<Vec<WorkspaceTarget>> {
        const KINDS: [WorkspaceTargetKind; 2] =
            [WorkspaceTargetKind::Library, WorkspaceTargetKind::Typecheck];
        let labels = self
            .packages
            .iter()
            .map(|(name, project)| {
                Ok((
                    name.clone(),
                    [
                        target_label(cell, project, WorkspaceTargetKind::Library)?,
                        target_label(cell, project, WorkspaceTargetKind::Typecheck)?,
                    ],
                ))
            })
            .collect::<bsmr_error::Result<BTreeMap<_, _>>>()?;
        let mut targets = Vec::with_capacity(labels.len() * KINDS.len());
        for (name, project) in &self.packages {
            for kind in KINDS {
                let label = labels.get(name).ok_or_else(|| {
                    internal_error!(
                        "lowered package `{name}` has no `{}` label",
                        kind.target_name()
                    )
                })?[kind.index()]
                .clone();
                let dependencies = project
                    .dependencies()
                    .map(|dependency| {
                        labels
                            .get(dependency)
                            .map(|labels| labels[kind.index()].clone())
                            .ok_or_else(|| {
                                internal_error!(
                                    "lowered dependency `{dependency}` has no `{}` label",
                                    kind.target_name()
                                )
                            })
                    })
                    .collect::<bsmr_error::Result<_>>()?;
                targets.push(WorkspaceTarget {
                    kind,
                    label,
                    dependencies,
                });
            }
        }
        Ok(targets)
    }
}

/// Derives one conventional capability target label from a workspace project.
fn target_label(
    cell: CellName,
    project: &WorkspaceProject,
    kind: WorkspaceTargetKind,
) -> bsmr_error::Result<TargetLabel> {
    let target_name = TargetName::new(kind.target_name())?;
    let package = PackageLabel::new(cell, &project.root)?;
    Ok(TargetLabel::new(package, target_name.as_ref()))
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
) -> Result<BTreeMap<String, WorkspaceProject>, WorkspaceGraphError> {
    packages
        .iter()
        .map(|(name, package)| {
            let dependencies = resolve_dependencies(package, packages)?;
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
) -> Result<BTreeMap<String, WorkspaceDependency>, WorkspaceGraphError> {
    let mut resolved = BTreeMap::new();
    for (dependency, declarations) in &package.dependencies {
        for declaration in declarations {
            let Some(range) = declaration.specifier.strip_prefix("workspace:") else {
                if packages.contains_key(dependency) {
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
    use bsmr_core::cells::name::CellName;
    use bsmr_core::cells::paths::CellRelativePathBuf;

    use super::DependencySection;
    use super::PnpmWorkspace;
    use super::WorkspaceGraph;
    use super::WorkspaceGraphError;
    use super::WorkspacePackage;
    use super::WorkspaceTargetKind;

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
            graph.package_names().collect::<Vec<_>>(),
            ["@acme/api", "@acme/core", "@acme/testkit"]
        );
        assert_eq!(
            graph.dependencies("@acme/api").unwrap().collect::<Vec<_>>(),
            ["@acme/core", "@acme/testkit"]
        );
        assert_eq!(graph.dependencies("@acme/core").unwrap().count(), 0);
        let sections = graph
            .package("@acme/api")
            .unwrap()
            .dependency("@acme/testkit")
            .unwrap();
        assert_eq!(sections.sections().count(), 1);
        assert_eq!(
            sections.specifier(DependencySection::DevDependency),
            Some("workspace:*")
        );
    }

    #[test]
    fn invariant_native_targets_use_bsmr_package_labels() {
        let app = package(
            "apps/api",
            r#"{
                "name": "@acme/api",
                "dependencies": {"@acme/core": "workspace:*"}
            }"#,
        );
        let core = package("packages/core", r#"{"name":"@acme/core"}"#);
        let graph = WorkspaceGraph::build([app, core]).unwrap();

        let targets = graph.lower(CellName::testing_new("root")).unwrap();

        assert_eq!(targets[0].kind(), WorkspaceTargetKind::Library);
        assert_eq!(targets[0].label().to_string(), "root//apps/api:lib");
        assert_eq!(
            targets[0]
                .dependencies()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            ["root//packages/core:lib"]
        );
        assert_eq!(targets[1].kind(), WorkspaceTargetKind::Typecheck);
        assert_eq!(targets[1].label().to_string(), "root//apps/api:typecheck");
        assert_eq!(
            targets[1]
                .dependencies()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            ["root//packages/core:typecheck"]
        );
        assert_eq!(targets[2].label().to_string(), "root//packages/core:lib");
        assert_eq!(
            targets[3].label().to_string(),
            "root//packages/core:typecheck"
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
    fn invariant_workspace_manifest_requires_explicit_package_patterns() {
        let workspace = PnpmWorkspace::parse(
            r#"
packages:
  - "apps/*"
  - packages/typescript/**
"#,
        )
        .unwrap();

        assert_eq!(workspace.patterns(), ["apps/*", "packages/typescript/**"]);
        assert!(PnpmWorkspace::parse("packages: []").is_err());
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
