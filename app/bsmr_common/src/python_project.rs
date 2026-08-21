//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Renders private Python rules from standard project metadata.

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use bsmr_core::cells::paths::CellRelativePathBuf;
use bsmr_core::fs::output_path::BSMR_OUTPUT_ROOT;
use bsmr_core::package::package_relative_path::PackageRelativePath;
use globset::GlobBuilder;
use globset::GlobSet;
use globset::GlobSetBuilder;

use crate::package_listing::listing::PackageListing;
mod build_requirements;
mod manifest;
mod render;

use build_requirements::manifest_build_requirements;
pub use build_requirements::validate_python_build_requirements;
use manifest::BuildConfigSetting;
use manifest::ExtraBuildDependency;
use manifest::Manifest;
use manifest::Project;
pub use render::render_python_build_file;

/// A native Python package cannot be lowered without satisfying these invariants.
#[derive(Debug, bsmr_error::Error)]
#[bsmr(tag = Input)]
pub enum NativePythonBuildError {
    /// Standard project metadata must declare an installable project.
    #[error("pyproject.toml must declare project.name and project.requires-python")]
    MissingProjectMetadata,
    /// BSMR's private target namespace must remain unavailable to ecosystem packages.
    #[error("Python project `{0}` conflicts with BSMR's reserved target name `{1}`")]
    ReservedTargetName(CellRelativePathBuf, String),
    /// Project metadata must remain valid TOML.
    #[error("Invalid pyproject.toml: {0}")]
    InvalidManifest(toml::de::Error),
    /// PEP 508 dependency names must be statically identifiable for workspace graph edges.
    #[error("invalid PEP 508 dependency `{0}`")]
    InvalidRequirement(String),
    /// Two workspace members cannot own the same normalized distribution name.
    #[error("duplicate Python workspace project `{0}`")]
    DuplicateWorkspaceProject(String),
    /// Dynamic metadata needs a supported backend adapter before analysis can be exact.
    #[error("Python project `{0}` has dynamic dependencies BSMR cannot resolve exactly")]
    UnsupportedDynamicDependencies(String),
    /// A declared Git-derived version requires an ordinary repository database.
    #[error("Python project declares Git-derived metadata but the repository has no .git/HEAD")]
    MissingVcsState,
    /// Every PEP 517 and compatibility requirement must be in the frozen build lock.
    #[error("pylock.build.toml does not contain required build package `{0}`")]
    MissingBuildRequirement(String),
    /// uv match-runtime packages must have one identical version in both locks.
    #[error("build package `{package}` has versions {build:?} but runtime lock has {runtime:?}")]
    BuildRequirementVersionMismatch {
        package: String,
        runtime: Vec<String>,
        build: Vec<String>,
    },
    /// A local build requirement would make the first-party wheel graph cyclic.
    #[error(
        "build package `{package}` is a local directory `{path}`; publish or vendor a wheel or source archive"
    )]
    LocalDirectoryBuildRequirement { package: String, path: String },
    /// Local lock entries must identify one project already represented in the workspace graph.
    #[error(
        "Python lock package `{package}` maps local directory `{path}` to no selected workspace project"
    )]
    UnknownLocalDirectorySource { package: String, path: String },
    /// Test profile names become target labels and therefore use one portable spelling.
    #[error(
        "Python test lock `{0}` must use pylock.test.toml or pylock.test-<name>.toml with lowercase ASCII letters, digits, and hyphens"
    )]
    InvalidTestLockName(String),
    /// A test runner is executable argv, never an empty or shell-interpreted string.
    #[error("[tool.bsmr.python].test-command must contain only nonempty argv entries")]
    InvalidTestCommand,
    /// uv workspace membership must be representable by BSMR's deterministic matcher.
    #[error("invalid uv workspace pattern `{pattern}`: {error}")]
    InvalidWorkspacePattern {
        pattern: String,
        error: globset::Error,
    },
    /// The compiled workspace selector exceeded the glob engine's limits.
    #[error("invalid uv workspace pattern set: {0}")]
    InvalidWorkspacePatternSet(globset::Error),
    /// Formatting internal Starlark into a string should be infallible.
    #[error("failed to render native Python build graph")]
    Render(std::fmt::Error),
}

/// Git metadata files that make dynamic project versions explicit action inputs.
pub struct PythonVcsFiles {
    /// Git-database paths relative to `.git` that the backend may read.
    pub files: Vec<String>,
}

/// Root-only Python files that alter which private graph nodes are available.
#[derive(Default)]
pub struct PythonRootFiles {
    /// Root configuration files inherited by nested native project actions.
    pub config_files: Vec<String>,
    /// Package-granular actions derived from the default runtime lock.
    pub runtime_packages: Vec<crate::python_lock::PylockInstallationFragment>,
    /// Package-granular actions derived from the isolated PEP 517 build lock.
    pub build_packages: Vec<crate::python_lock::PylockInstallationFragment>,
    /// First-party wheels overlaid into each runnable environment.
    pub members: Vec<PythonWorkspaceMember>,
    /// Named dependency closures that become generated test targets.
    pub test_locks: Vec<PythonTestLock>,
    /// Declared Git inputs for projects whose version is computed dynamically.
    pub vcs: Option<PythonVcsFiles>,
}

/// Returns whether the root PEP 751 lock activates native Python analysis.
#[must_use]
pub fn is_native_python_workspace(listing: &PackageListing) -> bool {
    listing
        .get_file(PackageRelativePath::unchecked_new("pylock.toml"))
        .is_some()
}

/// Returns standard root configurations discovered by Ruff and ty ancestor traversal.
pub fn python_root_config_files(listing: &PackageListing) -> Vec<String> {
    listing
        .files()
        .files()
        .map(PackageRelativePath::as_str)
        .filter(|path| {
            matches!(
                *path,
                ".ruff.toml" | "pyproject.toml" | "ruff.toml" | "ty.toml"
            )
        })
        .map(str::to_owned)
        .collect()
}

/// One named PEP 751 installation set exposed as a native test target.
pub struct PythonTestLock {
    /// Private target that materializes the lock into a CAS directory.
    pub environment: String,
    /// Root-relative PEP 751 lock consumed by the environment action.
    pub file: String,
    /// Package-granular actions derived from this named lock.
    pub packages: Vec<crate::python_lock::PylockInstallationFragment>,
    /// Public test label derived from the lock's portable profile name.
    pub target: String,
}

/// One first-party distribution available to package-local runtime environments.
#[derive(Clone, Debug)]
pub struct PythonWorkspaceMember {
    /// Root-relative package containing the member project.
    pub package: String,
    /// Canonical wheel target generated from the member's project name.
    pub target: String,
    /// PEP 508 dependency requirements declared by the member's effective metadata.
    pub dependencies: Vec<String>,
    /// Optional PEP 508 requirements keyed by normalized extra name.
    pub optional_dependencies: BTreeMap<String, Vec<String>>,
    /// Build packages required before this member's backend executes.
    pub build_requirements: Vec<String>,
    /// Whether the build backend computes dependencies dynamically.
    pub dynamic_dependencies: bool,
    /// Whether the build backend computes optional dependencies dynamically.
    pub dynamic_optional_dependencies: bool,
}

/// Returns the canonical target name when a manifest declares a Python project.
#[must_use = "the generated target name determines whether this manifest joins the build graph"]
pub fn python_project_name(manifest: &str) -> Result<Option<String>, NativePythonBuildError> {
    let manifest =
        toml::from_str::<Manifest>(manifest).map_err(NativePythonBuildError::InvalidManifest)?;
    manifest_project_name(manifest.project.as_ref())
}

/// Returns the wheel target for a project that uv considers installable.
#[must_use = "workspace environments must contain only installable distributions"]
pub fn python_distribution_name(manifest: &str) -> Result<Option<String>, NativePythonBuildError> {
    let manifest =
        toml::from_str::<Manifest>(manifest).map_err(NativePythonBuildError::InvalidManifest)?;
    if manifest.tool.uv.package == Some(false) {
        return Ok(None);
    }
    manifest_project_name(manifest.project.as_ref())
}

/// Returns whether a declared dynamic-version provider consumes Git state.
#[must_use = "VCS-derived wheels must declare their Git inputs"]
pub fn python_project_uses_vcs(manifest: &str) -> Result<bool, NativePythonBuildError> {
    let manifest =
        toml::from_str::<Manifest>(manifest).map_err(NativePythonBuildError::InvalidManifest)?;
    Ok(manifest_uses_vcs(&manifest))
}

/// Returns whether the parsed project delegates its dynamic version to Git.
fn manifest_uses_vcs(manifest: &Manifest) -> bool {
    let dynamic_version = manifest
        .project
        .as_ref()
        .is_some_and(|project| project.dynamic.iter().any(|field| field == "version"));
    if !dynamic_version {
        return false;
    }
    manifest.tool.uv_dynamic_versioning.is_some()
        || manifest.tool.setuptools_scm.is_some()
        || manifest.tool.uv.cache_keys.iter().any(|key| key.uses_git())
        || manifest
            .tool
            .hatch
            .version
            .as_ref()
            .and_then(|version| version.source.as_deref())
            .is_some_and(|source| matches!(source, "vcs" | "uv-dynamic-versioning"))
}

/// Parses one installable uv workspace member and its first-party edge candidates.
#[must_use = "workspace metadata defines the first-party dependency graph"]
pub fn python_workspace_member(
    package: String,
    manifest: &str,
) -> Result<Option<PythonWorkspaceMember>, NativePythonBuildError> {
    let manifest =
        toml::from_str::<Manifest>(manifest).map_err(NativePythonBuildError::InvalidManifest)?;
    if manifest.tool.uv.package == Some(false) {
        return Ok(None);
    }
    let Some(project) = manifest.project.as_ref() else {
        return Ok(None);
    };
    let target = manifest_project_name(Some(project))?
        .ok_or(NativePythonBuildError::MissingProjectMetadata)?;
    let dynamic_dependencies = project.dynamic.iter().any(|field| field == "dependencies");
    let dynamic_optional_dependencies = project
        .dynamic
        .iter()
        .any(|field| field == "optional-dependencies");
    Ok(Some(PythonWorkspaceMember {
        package,
        target,
        dependencies: manifest_dependencies(&manifest)
            .map(<[String]>::to_vec)
            .unwrap_or_default(),
        optional_dependencies: manifest_optional_dependencies(&manifest)
            .map(|dependencies| {
                dependencies
                    .iter()
                    .map(|(extra, requirements)| {
                        (normalize_project_name(extra), requirements.to_owned())
                    })
                    .collect()
            })
            .unwrap_or_default(),
        build_requirements: manifest_build_requirements(&manifest)?,
        dynamic_dependencies: dynamic_dependencies && manifest_dependencies(&manifest).is_none(),
        dynamic_optional_dependencies: dynamic_optional_dependencies
            && manifest_optional_dependencies(&manifest).is_none(),
    }))
}

/// Selects the transitive first-party wheel closure for one Python project.
#[must_use = "runtime environments must contain the complete first-party closure"]
pub fn python_workspace_closure(
    manifest: &str,
    members: &[PythonWorkspaceMember],
) -> Result<Vec<PythonWorkspaceMember>, NativePythonBuildError> {
    let manifest =
        toml::from_str::<Manifest>(manifest).map_err(NativePythonBuildError::InvalidManifest)?;
    let Some(project) = manifest.project.as_ref() else {
        return Ok(Vec::new());
    };
    let project_name = normalize_project_name(&project.name);
    let mut by_name = BTreeMap::new();
    for member in members {
        if by_name.insert(member.target.as_str(), member).is_some() {
            return Err(NativePythonBuildError::DuplicateWorkspaceProject(
                member.target.to_owned(),
            ));
        }
    }
    let dependencies = manifest_dependencies(&manifest).ok_or_else(|| {
        NativePythonBuildError::UnsupportedDynamicDependencies(project_name.clone())
    })?;
    let mut selected = BTreeSet::new();
    let mut selected_extras = BTreeMap::<String, BTreeSet<String>>::new();
    let mut pending = workspace_requirements(dependencies)?;
    while let Some(requirement) = pending.pop() {
        let Some(member) = by_name.get(requirement.name.as_str()) else {
            continue;
        };
        let first = selected.insert(member.target.as_str());
        let extras = selected_extras.entry(member.target.clone()).or_default();
        let new_extras = requirement
            .extras
            .into_iter()
            .filter(|extra| extras.insert(extra.clone()))
            .collect::<Vec<_>>();
        if !first && new_extras.is_empty() {
            continue;
        }
        if member.dynamic_dependencies {
            return Err(NativePythonBuildError::UnsupportedDynamicDependencies(
                member.target.clone(),
            ));
        }
        if first {
            pending.extend(workspace_requirements(&member.dependencies)?);
        }
        if member.dynamic_optional_dependencies && !new_extras.is_empty() {
            return Err(NativePythonBuildError::UnsupportedDynamicDependencies(
                member.target.clone(),
            ));
        }
        for extra in new_extras {
            if let Some(dependencies) = member.optional_dependencies.get(&extra) {
                pending.extend(workspace_requirements(dependencies)?);
            }
        }
    }
    Ok(members
        .iter()
        .filter(|member| member.target != project_name && selected.contains(member.target.as_str()))
        .cloned()
        .collect())
}

/// One workspace edge extracted from the name and extras of a PEP 508 requirement.
struct WorkspaceRequirement {
    name: String,
    extras: BTreeSet<String>,
}

/// Returns the exact base dependencies exposed by static PEP 621 or Hatch metadata.
fn manifest_dependencies(manifest: &Manifest) -> Option<&[String]> {
    let project = manifest.project.as_ref()?;
    if !project.dynamic.iter().any(|field| field == "dependencies") {
        return Some(&project.dependencies);
    }
    manifest
        .tool
        .hatch
        .metadata
        .as_ref()?
        .hooks
        .uv_dynamic_versioning
        .as_ref()
        .map(|metadata| metadata.dependencies.as_slice())
}

/// Returns exact optional dependencies exposed by static PEP 621 or Hatch metadata.
fn manifest_optional_dependencies(manifest: &Manifest) -> Option<&BTreeMap<String, Vec<String>>> {
    let project = manifest.project.as_ref()?;
    if !project
        .dynamic
        .iter()
        .any(|field| field == "optional-dependencies")
    {
        return Some(&project.optional_dependencies);
    }
    manifest
        .tool
        .hatch
        .metadata
        .as_ref()?
        .hooks
        .uv_dynamic_versioning
        .as_ref()
        .map(|metadata| &metadata.optional_dependencies)
}

/// Parses only the distribution identity and requested extras needed by the workspace graph.
fn workspace_requirements(
    requirements: &[String],
) -> Result<Vec<WorkspaceRequirement>, NativePythonBuildError> {
    requirements
        .iter()
        .map(|requirement| {
            let requirement = requirement.trim_start();
            let name_end = requirement
                .char_indices()
                .take_while(|(_, character)| {
                    character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
                })
                .last()
                .map(|(index, character)| index + character.len_utf8())
                .filter(|_| {
                    requirement
                        .as_bytes()
                        .first()
                        .is_some_and(|byte| byte.is_ascii_alphanumeric())
                })
                .ok_or_else(|| {
                    NativePythonBuildError::InvalidRequirement(requirement.to_owned())
                })?;
            let mut remainder = requirement[name_end..].trim_start();
            let mut extras = BTreeSet::new();
            if let Some(after_open) = remainder.strip_prefix('[') {
                let (contents, after_close) = after_open.split_once(']').ok_or_else(|| {
                    NativePythonBuildError::InvalidRequirement(requirement.to_owned())
                })?;
                for extra in contents.split(',') {
                    let extra = extra.trim();
                    if extra.is_empty()
                        || !extra.chars().all(|character| {
                            character.is_ascii_alphanumeric()
                                || matches!(character, '-' | '_' | '.')
                        })
                    {
                        return Err(NativePythonBuildError::InvalidRequirement(
                            requirement.to_owned(),
                        ));
                    }
                    extras.insert(normalize_project_name(extra));
                }
                remainder = after_close.trim_start();
            }
            if remainder.starts_with(']') {
                return Err(NativePythonBuildError::InvalidRequirement(
                    requirement.to_owned(),
                ));
            }
            Ok(WorkspaceRequirement {
                name: normalize_project_name(&requirement[..name_end]),
                extras,
            })
        })
        .collect()
}

/// Validates and normalizes one parsed PEP 621 project name.
fn manifest_project_name(
    project: Option<&Project>,
) -> Result<Option<String>, NativePythonBuildError> {
    project
        .map(|project| {
            project
                .requires_python
                .as_ref()
                .ok_or(NativePythonBuildError::MissingProjectMetadata)?;
            Ok(normalize_project_name(&project.name))
        })
        .transpose()
}

/// Returns canonical distribution names from PEP 508 dependency strings.
fn requirement_names(requirements: &[String]) -> Result<Vec<String>, NativePythonBuildError> {
    Ok(workspace_requirements(requirements)?
        .into_iter()
        .map(|requirement| requirement.name)
        .collect())
}

/// Discovers deterministic test profiles from root-level PEP 751 lock names.
#[must_use = "test locks must be rendered into the generated build graph"]
pub fn python_test_locks(
    listing: &PackageListing,
) -> Result<Vec<PythonTestLock>, NativePythonBuildError> {
    let mut locks = listing
        .files()
        .files()
        .map(PackageRelativePath::as_str)
        .filter(|file| {
            !file.contains('/') && (*file == "pylock.test.toml" || file.starts_with("pylock.test-"))
        })
        .map(|file| {
            let target = if file == "pylock.test.toml" {
                "test"
            } else {
                file.strip_prefix("pylock.")
                    .and_then(|name| name.strip_suffix(".toml"))
                    .filter(|name| name.starts_with("test-") && name.len() > 5)
                    .filter(|name| {
                        name.bytes().all(|byte| {
                            byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-'
                        })
                    })
                    .ok_or_else(|| NativePythonBuildError::InvalidTestLockName(file.to_owned()))?
            };
            Ok(PythonTestLock {
                environment: format!("__bsmr_python_{target}_environment"),
                file: file.to_owned(),
                packages: Vec::new(),
                target: target.to_owned(),
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    locks.sort_by(|left, right| {
        (left.target != "test", &left.target).cmp(&(right.target != "test", &right.target))
    });
    Ok(locks)
}

/// Returns nested project manifests after pruning structurally detected virtual environments.
#[must_use = "workspace manifests must be lowered into first-party wheel dependencies"]
pub fn python_workspace_manifest_paths<'a>(
    manifest: &str,
    listing: &'a PackageListing,
) -> Result<Vec<&'a str>, NativePythonBuildError> {
    let manifest =
        toml::from_str::<Manifest>(manifest).map_err(NativePythonBuildError::InvalidManifest)?;
    let Some(workspace) = manifest.tool.uv.workspace else {
        return Ok(Vec::new());
    };
    let members = compile_workspace_patterns(&workspace.members)?;
    let exclusions = compile_workspace_patterns(&workspace.exclude)?;
    let virtual_environments = virtual_environment_roots(listing);
    Ok(listing
        .files()
        .files()
        .map(PackageRelativePath::as_str)
        .filter(|path| *path != "pyproject.toml" && path.ends_with("/pyproject.toml"))
        .filter(|path| !is_generated_path(path, &virtual_environments))
        .filter(|path| {
            let root = path
                .strip_suffix("/pyproject.toml")
                .expect("filtered workspace manifest path");
            members.is_match(root) && !exclusions.is_match(root)
        })
        .collect())
}

/// Compiles uv workspace member and exclusion patterns with path-aware glob semantics.
fn compile_workspace_patterns(patterns: &[String]) -> Result<GlobSet, NativePythonBuildError> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        let glob = GlobBuilder::new(pattern)
            .literal_separator(true)
            .backslash_escape(false)
            .build()
            .map_err(|error| NativePythonBuildError::InvalidWorkspacePattern {
                pattern: pattern.to_owned(),
                error,
            })?;
        builder.add(glob);
    }
    builder
        .build()
        .map_err(NativePythonBuildError::InvalidWorkspacePatternSet)
}

/// Converts a distribution name into its canonical PEP 503 spelling.
fn normalize_project_name(name: &str) -> String {
    let mut normalized = String::with_capacity(name.len());
    let mut separator = false;
    for character in name.chars() {
        if matches!(character, '-' | '_' | '.') {
            separator = true;
        } else {
            if separator && !normalized.is_empty() {
                normalized.push('-');
            }
            separator = false;
            normalized.extend(character.to_lowercase());
        }
    }
    normalized
}

/// Rejects a test command that cannot name a Python program or module.
fn validate_test_command(command: Option<&[String]>) -> Result<(), NativePythonBuildError> {
    if command.is_some_and(|command| {
        command.is_empty() || command.iter().any(|argument| argument.is_empty())
    }) {
        return Err(NativePythonBuildError::InvalidTestCommand);
    }
    Ok(())
}

/// Selects source-controlled project files and rejects generated state.
fn project_files(listing: &PackageListing, analysis_only: bool) -> impl Iterator<Item = &str> {
    let virtual_environments = virtual_environment_roots(listing);
    listing
        .files()
        .files()
        .map(PackageRelativePath::as_str)
        .filter(move |path| !is_generated_path(path, &virtual_environments))
        .filter(move |path| !analysis_only || is_analysis_input(path))
}

/// Rejects mutable outputs regardless of where they occur in a project tree.
fn is_generated_path(path: &str, virtual_environments: &[String]) -> bool {
    virtual_environments.iter().any(|root| {
        path == root
            || path
                .strip_prefix(root)
                .is_some_and(|suffix| suffix.starts_with('/'))
    }) || path
        .split('/')
        .next()
        .is_some_and(|component| component.starts_with("bazel-"))
        || (path.starts_with("pylock.") && path.ends_with(".toml"))
        || path == "pylock.toml"
        || path == "uv.lock"
        || path.split('/').any(|component| {
            component == BSMR_OUTPUT_ROOT
                || matches!(
                    component,
                    ".bsmr"
                        | ".git"
                        | ".mypy_cache"
                        | ".pytest_cache"
                        | ".ruff_cache"
                        | ".venv"
                        | "__pycache__"
                        | "build"
                        | "dist"
                        | "node_modules"
                        | "target"
                        | "target-bsmr"
                )
                || component.ends_with(".egg-info")
        })
}

/// Finds environment roots by the standard file every Python virtual environment carries.
fn virtual_environment_roots(listing: &PackageListing) -> Vec<String> {
    listing
        .files()
        .files()
        .map(PackageRelativePath::as_str)
        .filter_map(|path| path.strip_suffix("/pyvenv.cfg"))
        .map(str::to_owned)
        .collect()
}

/// Narrows lint and typecheck keys to semantic Python and tool configuration.
fn is_analysis_input(path: &str) -> bool {
    path.ends_with(".py")
        || path.ends_with(".pyi")
        || path.ends_with(".ipynb")
        || matches!(
            path.rsplit('/').next(),
            Some(".gitignore" | ".ruff.toml" | "pyproject.toml" | "ruff.toml" | "ty.toml")
        )
}

/// Joins a package-local file to its workspace-relative path.
fn workspace_path(package_root: &CellRelativePathBuf, file: &str) -> String {
    if package_root.is_empty() {
        file.to_owned()
    } else {
        format!("{package_root}/{file}")
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use bsmr_core::cells::paths::CellRelativePathBuf;

    use super::NativePythonBuildError;
    use super::PythonRootFiles;
    use super::PythonTestLock;
    use super::PythonVcsFiles;
    use super::PythonWorkspaceMember;
    use super::is_native_python_workspace;
    use super::python_distribution_name;
    use super::python_project_name;
    use super::python_project_uses_vcs;
    use super::python_root_config_files;
    use super::python_test_locks;
    use super::python_workspace_closure;
    use super::python_workspace_manifest_paths;
    use super::python_workspace_member;
    use super::render_python_build_file;
    use super::validate_python_build_requirements;
    use crate::package_listing::listing::PackageListing;
    use crate::package_listing::listing::testing::PackageListingExt;
    use crate::python_lock::PylockAcquisition;
    use crate::python_lock::PylockArtifact;
    use crate::python_lock::PylockInstallationFragment;
    use crate::python_lock::PylockToml;

    fn package_fragments(packages: &[&str]) -> Vec<PylockInstallationFragment> {
        packages
            .iter()
            .map(|package| PylockInstallationFragment {
                package: (*package).to_owned(),
                contents: format!(
                    "lock-version = '1.0'\ncreated-by = 'test'\n[[packages]]\nname = '{package}'\n"
                ),
                acquisition: PylockAcquisition::Wheel,
                artifact: None,
                platform_artifacts: BTreeMap::new(),
                source_artifact: None,
                vcs_source: None,
                directory_source: None,
                artifacts: BTreeMap::new(),
            })
            .collect()
    }

    fn wheel_lock(packages: &[(&str, &str)]) -> PylockToml {
        let mut contents = "lock-version = '1.0'\ncreated-by = 'test'\n".to_owned();
        for (name, version) in packages {
            contents.push_str(&format!(
                "[[packages]]\nname = '{name}'\nversion = '{version}'\n[[packages.wheels]]\nurl = 'https://example.org/{name}-{version}-py3-none-any.whl'\nhashes = {{ sha256 = '0000000000000000000000000000000000000000000000000000000000000000' }}\n"
            ));
        }
        PylockToml::parse(&contents).unwrap()
    }

    fn root_files_with_packages(packages: &[&str]) -> PythonRootFiles {
        PythonRootFiles {
            runtime_packages: package_fragments(packages),
            build_packages: package_fragments(packages),
            ..PythonRootFiles::default()
        }
    }

    fn root_files_with_source_package(package: &str) -> PythonRootFiles {
        let mut files = root_files_with_packages(&[package]);
        files.runtime_packages[0].acquisition = PylockAcquisition::Source;
        files
    }

    fn root_files_with_source_packages(packages: &[&str]) -> PythonRootFiles {
        let mut files = PythonRootFiles {
            runtime_packages: package_fragments(packages),
            ..PythonRootFiles::default()
        };
        for package in &mut files.runtime_packages {
            package.acquisition = PylockAcquisition::Source;
        }
        files
    }

    #[test]
    fn invariant_tool_only_manifests_are_not_workspace_projects() {
        let project = python_project_name("[tool.ruff]\nline-length = 88\n").unwrap();

        assert_eq!(project, None);
    }

    #[test]
    fn design_pep_751_lock_activates_native_python() {
        let unlocked = PackageListing::testing_files(&["pyproject.toml"]);
        let locked = PackageListing::testing_files(&["pylock.toml", "pyproject.toml"]);

        assert!(!is_native_python_workspace(&unlocked));
        assert!(is_native_python_workspace(&locked));
    }

    #[test]
    fn invariant_tool_only_root_hosts_workspace_infrastructure() {
        let listing = PackageListing::testing_files(&["pyproject.toml"]);
        let build = render_python_build_file(
            CellRelativePathBuf::unchecked_new(String::new()),
            "[tool.ruff]\nline-length = 88\n",
            &listing,
            &PythonRootFiles {
                config_files: Vec::new(),
                runtime_packages: Vec::new(),
                build_packages: Vec::new(),
                members: vec![PythonWorkspaceMember {
                    package: "packages/member".to_owned(),
                    target: "member".to_owned(),
                    dependencies: Vec::new(),
                    optional_dependencies: Default::default(),
                    build_requirements: Vec::new(),
                    dynamic_dependencies: false,
                    dynamic_optional_dependencies: false,
                }],
                test_locks: Vec::new(),
                vcs: None,
            },
        )
        .unwrap();

        assert!(build.contains("python_native_toolchain()"));
        assert!(build.contains("name = \"__bsmr_python_environment\""));
        assert!(build.contains("\"//packages/member:member\""));
        assert!(!build.contains("python_wheel(\n"));
        assert!(!build.contains("name = \"lint\""));
    }

    #[test]
    fn invariant_non_package_projects_run_checks_without_building_a_wheel() {
        let listing = PackageListing::testing_files(&["pyproject.toml", "src/runtime/__init__.py"]);
        let build = render_python_build_file(
            CellRelativePathBuf::unchecked_new("apps/runtime".to_owned()),
            "[project]\nname = 'runtime'\nversion = '1'\nrequires-python = '>=3.14'\n[tool.uv]\npackage = false\n",
            &listing,
            &PythonRootFiles::default(),
        )
        .unwrap();

        assert!(build.contains("name = \"lint\""));
        assert!(build.contains("name = \"typecheck\""));
        assert!(!build.contains("python_wheel(\n"));
        assert_eq!(
            python_distribution_name(
                "[project]\nname = 'runtime'\nrequires-python = '>=3.14'\n[tool.uv]\npackage = false\n"
            )
            .unwrap(),
            None
        );
    }

    #[test]
    fn invariant_workspace_projects_declare_an_interpreter_contract() {
        let project = python_project_name("[project]\nname = 'demo'\n");

        assert!(matches!(
            project,
            Err(NativePythonBuildError::MissingProjectMetadata)
        ));
    }

    #[test]
    fn invariant_scripts_cannot_claim_generated_target_names() {
        let listing = PackageListing::testing_files(&["pyproject.toml", "demo.py"]);
        let build = render_python_build_file(
            CellRelativePathBuf::unchecked_new(String::new()),
            "[project]\nname = 'demo'\nversion = '1'\nrequires-python = '>=3.12'\n[project.scripts]\nlint = 'demo:main'\n",
            &listing,
            &PythonRootFiles::default(),
        );

        assert!(matches!(
            build,
            Err(NativePythonBuildError::ReservedTargetName(_, target)) if target == "lint"
        ));
    }

    #[test]
    fn invariant_root_project_gets_standard_zero_configuration_targets() {
        let listing = PackageListing::testing_files(&[
            ".ruff.toml",
            "pyproject.toml",
            "pylock.toml",
            "ruff.toml",
            "src/demo/__init__.py",
            "src/demo/__pycache__/ignored.pyc",
            "ty.toml",
            "uv.toml",
            "examples/pyproject.toml",
            "README.md",
            "bazel-demo/generated.py",
            "bsmr-out/generated.py",
            ".venv/ignored.py",
            ".custom-env/pyvenv.cfg",
            ".custom-env/lib/python3.14/site-packages/demo/pyproject.toml",
            ".custom-env/lib/python3.14/site-packages/demo/__init__.py",
            "node_modules/tool/index.py",
            "target/generated.py",
            "target-bsmr/environment/package.py",
        ]);
        let build = render_python_build_file(
            CellRelativePathBuf::unchecked_new(String::new()),
            "[project]\nname = 'Demo_Project'\nrequires-python = '>=3.12'\n",
            &listing,
            &root_files_with_source_package("demo"),
        )
        .unwrap();

        assert!(build.contains("name = \"demo-project\""));
        assert!(build.contains("name = \"lint\""));
        assert!(build.contains("name = \"typecheck\""));
        assert!(build.contains("python_environment("));
        assert!(build.contains("python_wheel_environment("));
        assert_eq!(build.matches("    wheels = [").count(), 1);
        assert!(build.contains("name = \"__bsmr_python_build_environment\""));
        assert!(build.contains("name = \"__bsmr_python_package__demo__"));
        assert!(build.contains("package = \"demo\""));
        assert_eq!(
            build
                .matches("build_environment = \":__bsmr_python_build_environment\"")
                .count(),
            1
        );
        assert!(build.contains("\"src/demo/__init__.py\""));
        assert!(!build.contains(".venv/ignored.py"));
        assert!(!build.contains(".custom-env"));
        assert!(!build.contains("bazel-demo"));
        assert!(!build.contains("bsmr-out"));
        assert!(!build.contains("node_modules"));
        assert!(!build.contains("target-bsmr"));
        assert!(!build.contains("target/generated.py"));
        assert!(!build.contains("__pycache__"));
        assert_eq!(build.matches("\"examples/pyproject.toml\"").count(), 4);
        assert_eq!(build.matches("\".ruff.toml\"").count(), 4);
        assert_eq!(build.matches("\"ruff.toml\"").count(), 4);
        assert_eq!(build.matches("\"ty.toml\"").count(), 4);
        assert_eq!(build.matches("\"uv.toml\"").count(), 2);
        assert_eq!(build.matches("\"README.md\"").count(), 2);
    }

    #[test]
    fn invariant_ruff_receives_files_selected_by_native_configuration() {
        let listing = PackageListing::testing_files(&[
            "pyproject.toml",
            "src/demo/__init__.py",
            "templates/check.j2",
        ]);
        let build = render_python_build_file(
            CellRelativePathBuf::unchecked_new(String::new()),
            "[project]\nname = 'demo'\nversion = '1'\nrequires-python = '>=3.14'\n[tool.ruff]\nextend-include = ['*.j2']\n",
            &listing,
            &root_files_with_source_package("demo"),
        )
        .unwrap();

        assert_eq!(build.matches("\"templates/check.j2\"").count(), 2);
        assert!(build.contains(
            "ruff_check(\n    name = \"lint\",\n    python = \"root//:__bsmr_python_distribution\",\n    project_root = \".\",\n    sources = \":__bsmr_python_sources\","
        ));
    }

    #[test]
    fn invariant_universal_wheels_are_declared_download_inputs() {
        let listing = PackageListing::testing_files(&["pyproject.toml"]);
        let mut root_files = root_files_with_packages(&["attrs"]);
        root_files.build_packages.clear();
        root_files.runtime_packages[0].artifact = Some(PylockArtifact {
            filename: "attrs-25.3.0-py3-none-any.whl".to_owned(),
            sha256: "0000000000000000000000000000000000000000000000000000000000000000".to_owned(),
            size: 42,
            url: "https://example.org/attrs-25.3.0-py3-none-any.whl".to_owned(),
        });

        let build = render_python_build_file(
            CellRelativePathBuf::unchecked_new(String::new()),
            "[project]\nname = 'demo'\nversion = '1'\nrequires-python = '>=3.14'\n",
            &listing,
            &root_files,
        )
        .unwrap();

        assert!(build.contains("python_locked_artifact(\n"));
        assert!(build.contains("filename = \"attrs-25.3.0-py3-none-any.whl\""));
        assert!(build.contains("size = 42"));
        assert!(build.contains("artifact = \":__bsmr_python_artifact__"));
    }

    #[test]
    fn invariant_source_distributions_are_declared_download_inputs() {
        let listing = PackageListing::testing_files(&["pyproject.toml"]);
        let lock = PylockToml::parse(&format!(
            "lock-version = '1.0'\ncreated-by = 'test'\n[[packages]]\nname = 'demo'\nversion = '1'\n[packages.sdist]\nurl = 'https://example.org/demo-1.tar.gz'\nsize = 42\nhashes = {{ sha256 = '{}' }}\n",
            "0".repeat(64),
        ))
        .unwrap();
        let root_files = PythonRootFiles {
            runtime_packages: lock.installation_fragments().unwrap(),
            ..PythonRootFiles::default()
        };

        let build = render_python_build_file(
            CellRelativePathBuf::unchecked_new(String::new()),
            "[project]\nname = 'demo'\nversion = '1'\nrequires-python = '>=3.14'\n",
            &listing,
            &root_files,
        )
        .unwrap();

        assert!(build.contains("filename = \"demo-1.tar.gz\""));
        assert!(build.contains("source_artifact = \":__bsmr_python_artifact__"));
    }

    #[test]
    fn invariant_remote_archives_preserve_their_locked_project_root() {
        let listing = PackageListing::testing_files(&["pyproject.toml"]);
        let lock = PylockToml::parse(&format!(
            "lock-version = '1.0'\ncreated-by = 'test'\n[[packages]]\nname = 'demo'\nversion = '1'\n[packages.archive]\nurl = 'https://example.org/source.zip'\nsubdirectory = 'python/package'\nsize = 42\nhashes = {{ sha256 = '{}' }}\n",
            "0".repeat(64),
        ))
        .unwrap();
        let root_files = PythonRootFiles {
            runtime_packages: lock.installation_fragments().unwrap(),
            ..PythonRootFiles::default()
        };

        let build = render_python_build_file(
            CellRelativePathBuf::unchecked_new(String::new()),
            "[project]\nname = 'demo'\nversion = '1'\nrequires-python = '>=3.14'\n",
            &listing,
            &root_files,
        )
        .unwrap();

        assert!(build.contains("filename = \"source.zip\""));
        assert!(build.contains("source_subdirectory = \"python/package\""));
        assert!(build.contains("source_version = \"1\""));
    }

    #[test]
    fn invariant_vcs_sources_are_acquired_at_their_locked_commit() {
        let listing = PackageListing::testing_files(&["pyproject.toml"]);
        let lock = PylockToml::parse(
            "lock-version = '1.0'\ncreated-by = 'test'\n[[packages]]\nname = 'demo'\nversion = '1'\n[packages.vcs]\ntype = 'git'\nurl = 'https://example.org/demo.git'\ncommit-id = '0000000000000000000000000000000000000000'\nsubdirectory = 'python/package'\n",
        )
        .unwrap();
        let root_files = PythonRootFiles {
            runtime_packages: lock.installation_fragments().unwrap(),
            ..PythonRootFiles::default()
        };

        let build = render_python_build_file(
            CellRelativePathBuf::unchecked_new(String::new()),
            "[project]\nname = 'demo'\nversion = '1'\nrequires-python = '>=3.14'\n",
            &listing,
            &root_files,
        )
        .unwrap();

        assert!(build.contains("git_fetch(\n"));
        assert!(build.contains("repo = \"https://example.org/demo.git\""));
        assert!(build.contains("rev = \"0000000000000000000000000000000000000000\""));
        assert!(build.contains("source_tree = \":__bsmr_python_vcs__"));
        assert!(build.contains("source_subdirectory = \"python/package\""));
        assert!(build.contains("source_version = \"1\""));
    }

    #[test]
    fn invariant_local_directories_reuse_declared_workspace_wheels() {
        let listing = PackageListing::testing_files(&["pyproject.toml"]);
        let lock = PylockToml::parse(
            "lock-version = '1.0'\ncreated-by = 'test'\n[[packages]]\nname = 'helper'\n[packages.directory]\npath = 'packages/helper'\neditable = true\n",
        )
        .unwrap();
        let helper = python_workspace_member(
            "packages/helper".to_owned(),
            "[project]\nname = 'helper'\nrequires-python = '>=3.14'\n",
        )
        .unwrap()
        .unwrap();
        let root_files = PythonRootFiles {
            runtime_packages: lock.installation_fragments().unwrap(),
            members: vec![helper],
            ..PythonRootFiles::default()
        };

        let build = render_python_build_file(
            CellRelativePathBuf::unchecked_new(String::new()),
            "[project]\nname = 'demo'\nversion = '1'\nrequires-python = '>=3.14'\ndependencies = ['helper']\n",
            &listing,
            &root_files,
        )
        .unwrap();

        assert!(build.contains("\"//packages/helper:helper\""));
        assert!(!build.contains("package = \"helper\""));
    }

    #[test]
    fn invariant_local_directories_require_declared_workspace_projects() {
        let listing = PackageListing::testing_files(&["pyproject.toml"]);
        let lock = PylockToml::parse(
            "lock-version = '1.0'\ncreated-by = 'test'\n[[packages]]\nname = 'helper'\n[packages.directory]\npath = 'vendor/helper'\n",
        )
        .unwrap();
        let root_files = PythonRootFiles {
            runtime_packages: lock.installation_fragments().unwrap(),
            ..PythonRootFiles::default()
        };

        let error = render_python_build_file(
            CellRelativePathBuf::unchecked_new(String::new()),
            "[project]\nname = 'demo'\nversion = '1'\nrequires-python = '>=3.14'\n",
            &listing,
            &root_files,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            NativePythonBuildError::UnknownLocalDirectorySource { package, path }
                if package == "helper" && path == "vendor/helper"
        ));
    }

    #[test]
    fn invariant_platform_wheels_are_selected_by_execution_platform() {
        let listing = PackageListing::testing_files(&["pyproject.toml"]);
        let lock = PylockToml::parse(
            "lock-version = '1.0'\ncreated-by = 'test'\n[[packages]]\nname = 'attrs'\nversion = '25.3.0'\n[packages.sdist]\nurl = 'https://example.org/attrs-25.3.0.tar.gz'\nhashes = { sha256 = 'source' }\n[[packages.wheels]]\nurl = 'https://example.org/attrs-25.3.0-cp314-cp314-macosx_13_0_arm64.whl'\nsize = 42\nhashes = { sha256 = '0000000000000000000000000000000000000000000000000000000000000000' }\n",
        )
        .unwrap();
        let root_files = PythonRootFiles {
            runtime_packages: lock.installation_fragments().unwrap(),
            ..PythonRootFiles::default()
        };

        let build = render_python_build_file(
            CellRelativePathBuf::unchecked_new(String::new()),
            "[project]\nname = 'demo'\nversion = '1'\nrequires-python = '>=3.14'\n",
            &listing,
            &root_files,
        )
        .unwrap();

        assert!(build.contains("python_native_python_platform_value({"));
        assert!(build.contains("\"3.14-macos-arm64\": [\n"));
        assert!(build.contains("artifacts = python_native_python_platform_value({"));
    }

    /// One unconditional best wheel per platform bypasses dynamic uv selection.
    #[test]
    fn invariant_best_platform_wheels_are_direct_artifacts() {
        let listing = PackageListing::testing_files(&["pyproject.toml"]);
        let lock = PylockToml::parse(
            "lock-version = '1.0'\ncreated-by = 'test'\n[[packages]]\nname = 'demo-wheel'\nversion = '1'\n[packages.sdist]\nurl = 'https://example.org/demo_wheel-1.tar.gz'\nsize = 42\nhashes = { sha256 = '2222222222222222222222222222222222222222222222222222222222222222' }\n[[packages.wheels]]\nurl = 'https://example.org/demo_wheel-1-cp314-cp314-macosx_13_0_arm64.whl'\nsize = 42\nhashes = { sha256 = '0000000000000000000000000000000000000000000000000000000000000000' }\n[[packages.wheels]]\nurl = 'https://example.org/demo_wheel-1-py3-none-any.whl'\nsize = 42\nhashes = { sha256 = '1111111111111111111111111111111111111111111111111111111111111111' }\n",
        )
        .unwrap();
        let root_files = PythonRootFiles {
            runtime_packages: lock.installation_fragments().unwrap(),
            ..PythonRootFiles::default()
        };

        let build = render_python_build_file(
            CellRelativePathBuf::unchecked_new(String::new()),
            "[project]\nname = 'demo'\nversion = '1'\nrequires-python = '>=3.14'\n",
            &listing,
            &root_files,
        )
        .unwrap();

        assert!(build.contains("artifact = python_native_python_platform_value({"));
        assert!(!build.contains("artifacts = python_native_python_platform_value({"));
        assert!(build.contains("    acquisition = \"wheel\","));
        assert!(!build.contains("    source_artifact = "));
    }

    #[test]
    fn invariant_each_action_depends_only_on_the_tools_it_consumes() {
        let listing = PackageListing::testing_files(&["pyproject.toml", "src/demo/__init__.py"]);
        let build = render_python_build_file(
            CellRelativePathBuf::unchecked_new(String::new()),
            "[project]\nname = 'demo'\nrequires-python = '>=3.12'\n",
            &listing,
            &root_files_with_packages(&["demo"]),
        )
        .unwrap();
        let body = |rule: &str| {
            build
                .split_once(&format!("{rule}(\n"))
                .unwrap()
                .1
                .split_once("\n)\n")
                .unwrap()
                .0
        };

        let wheel = body("python_wheel");
        assert!(wheel.contains("distribution = \"demo\""));
        assert!(wheel.contains("environment = \"root//:__bsmr_python_build_environment\""));
        assert!(!wheel.contains("__bsmr_uv_distribution"));
        assert!(!wheel.contains("__bsmr_ruff_distribution"));
        assert!(!wheel.contains("__bsmr_ty_distribution"));

        let lint = body("ruff_check");
        assert!(lint.contains("__bsmr_ruff_distribution"));
        assert!(!lint.contains("environment ="));
        assert!(!lint.contains("__bsmr_uv_distribution"));
        assert!(!lint.contains("__bsmr_ty_distribution"));

        let typecheck = body("ty_check");
        assert!(typecheck.contains("environments ="));
        assert!(typecheck.contains("root//:__bsmr_python_environment"));
        assert!(!typecheck.contains("__bsmr_python_workspace_environment"));
        assert!(typecheck.contains("__bsmr_ty_distribution"));
        assert!(!typecheck.contains("__bsmr_uv_distribution"));
        assert!(!typecheck.contains("__bsmr_ruff_distribution"));

        let workspace = body("python_wheel_environment");
        assert!(workspace.contains("__bsmr_python_distribution"));
        assert!(!workspace.contains("__bsmr_uv_distribution"));
    }

    #[test]
    fn invariant_pep517_config_settings_are_typed_action_inputs() {
        let listing = PackageListing::testing_files(&["pyproject.toml", "src/demo/__init__.py"]);
        let build = render_python_build_file(
            CellRelativePathBuf::unchecked_new(String::new()),
            "[project]\nname = 'demo'\nrequires-python = '>=3.12'\n[tool.uv]\nconfig-settings = { editable-mode = 'strict', build-option = ['one', 'two'] }\n",
            &listing,
            &root_files_with_source_packages(&["demo", "numpy"]),
        )
        .unwrap();

        assert_eq!(build.matches("    config_settings = {\n").count(), 3);
        assert_eq!(
            build
                .matches("        \"build-option\": [\"one\", \"two\"],\n")
                .count(),
            3
        );
        assert_eq!(
            build
                .matches("        \"editable-mode\": [\"strict\"],\n")
                .count(),
            3
        );
    }

    #[test]
    fn invariant_pep517_config_settings_reject_untyped_values() {
        let listing = PackageListing::testing_files(&["pyproject.toml"]);
        let build = render_python_build_file(
            CellRelativePathBuf::unchecked_new(String::new()),
            "[project]\nname = 'demo'\nrequires-python = '>=3.12'\n[tool.uv]\nconfig-settings = { jobs = 4 }\n",
            &listing,
            &PythonRootFiles::default(),
        );

        assert!(matches!(
            build,
            Err(NativePythonBuildError::InvalidManifest(_))
        ));
    }

    #[test]
    fn invariant_package_build_settings_are_typed_action_inputs() {
        let listing = PackageListing::testing_files(&["pyproject.toml"]);
        let build = render_python_build_file(
            CellRelativePathBuf::unchecked_new(String::new()),
            "[project]\nname = 'demo'\nrequires-python = '>=3.12'\n[tool.uv.config-settings-package.numpy]\nsetup-args = ['-Dblas=blas', '-Dlapack=lapack']\n[tool.uv.config-settings-package.demo]\neditable-mode = 'compat'\n",
            &listing,
            &root_files_with_source_packages(&["demo", "numpy"]),
        )
        .unwrap();

        assert_eq!(
            build
                .matches("        \"numpy:setup-args=-Dblas=blas\",\n")
                .count(),
            1
        );
        assert_eq!(
            build
                .matches("        \"demo:editable-mode=compat\",\n")
                .count(),
            2
        );
    }

    #[test]
    fn invariant_package_build_variables_are_typed_action_inputs() {
        let listing = PackageListing::testing_files(&["pyproject.toml"]);
        let build = render_python_build_file(
            CellRelativePathBuf::unchecked_new(String::new()),
            "[project]\nname = 'demo'\nrequires-python = '>=3.12'\n[tool.uv.extra-build-variables.numpy]\nNPY_DISABLE_CPU_FEATURES = 'AVX512'\n[tool.uv.extra-build-variables.demo]\nDEMO_BUILD = 'strict'\n",
            &listing,
            &root_files_with_source_packages(&["demo", "numpy"]),
        )
        .unwrap();

        assert_eq!(
            build
                .matches("        \"numpy:NPY_DISABLE_CPU_FEATURES=AVX512\",\n")
                .count(),
            1
        );
        assert_eq!(
            build
                .matches("        \"demo:DEMO_BUILD=strict\",\n")
                .count(),
            2
        );
    }

    #[test]
    fn invariant_dynamic_versions_depend_on_declared_git_state() {
        let listing = PackageListing::testing_files(&["pyproject.toml", "src/demo/__init__.py"]);
        let build = render_python_build_file(
            CellRelativePathBuf::unchecked_new(String::new()),
            "[project]\nname = 'demo'\ndynamic = ['version']\nrequires-python = '>=3.12'\n[tool.hatch.version]\nsource = 'uv-dynamic-versioning'\n",
            &listing,
            &PythonRootFiles {
                config_files: Vec::new(),
                runtime_packages: Vec::new(),
                build_packages: Vec::new(),
                members: Vec::new(),
                test_locks: Vec::new(),
                vcs: Some(PythonVcsFiles {
                    files: vec![
                        "HEAD".to_owned(),
                        "objects/pack/demo.idx".to_owned(),
                        "objects/pack/demo.pack".to_owned(),
                        "packed-refs".to_owned(),
                    ],
                }),
            },
        )
        .unwrap();

        assert!(build.contains("python_vcs("));
        assert!(build.contains("\"objects/pack/demo.pack\": \".git/objects/pack/demo.pack\""));
        assert!(build.contains("\"packed-refs\": \".git/packed-refs\""));
        assert!(!build.contains("\"index\": \".git/index\""));
        assert!(!build.contains("\"shallow\": \".git/shallow\""));
        assert!(build.contains("vcs = \"root//:__bsmr_python_vcs\""));
    }

    #[test]
    fn invariant_only_declared_vcs_version_providers_receive_git_state() {
        assert!(!python_project_uses_vcs(
            "[project]\nname = 'demo'\ndynamic = ['version']\nrequires-python = '>=3.12'\n[tool.setuptools.dynamic]\nversion = { attr = 'demo.__version__' }\n",
        )
        .unwrap());
        assert!(python_project_uses_vcs(
            "[project]\nname = 'demo'\ndynamic = ['version']\nrequires-python = '>=3.12'\n[tool.hatch.version]\nsource = 'uv-dynamic-versioning'\n[tool.uv-dynamic-versioning]\nvcs = 'git'\n",
        )
        .unwrap());
        assert!(python_project_uses_vcs(
            "[project]\nname = 'demo'\ndynamic = ['version']\nrequires-python = '>=3.12'\n[tool.setuptools.dynamic]\nversion = { attr = 'demo.__version__' }\n[tool.uv]\ncache-keys = [{ git = { commit = true } }]\n",
        )
        .unwrap());
        assert!(python_project_uses_vcs(
            "[project]\nname = 'demo'\ndynamic = ['version']\nrequires-python = '>=3.12'\n[tool.setuptools.dynamic]\nversion = { attr = 'demo.__version__' }\n[tool.uv]\ncache-keys = [{ git = { tags = true } }]\n",
        )
        .unwrap());
        assert!(!python_project_uses_vcs(
            "[project]\nname = 'demo'\ndynamic = ['version']\nrequires-python = '>=3.12'\n[tool.setuptools.dynamic]\nversion = { attr = 'demo.__version__' }\n[tool.uv]\ncache-keys = [{ file = 'pyproject.toml' }, { git = { commit = false, tags = false } }]\n",
        )
        .unwrap());
    }

    #[test]
    fn invariant_dynamic_non_vcs_versions_do_not_reference_an_absent_vcs_target() {
        let listing = PackageListing::testing_files(&["pyproject.toml", "demo/__init__.py"]);
        let build = render_python_build_file(
            CellRelativePathBuf::unchecked_new(String::new()),
            "[project]\nname = 'demo'\ndynamic = ['version']\nrequires-python = '>=3.12'\n[tool.setuptools.dynamic]\nversion = { attr = 'demo.__version__' }\n",
            &listing,
            &PythonRootFiles::default(),
        )
        .unwrap();

        assert!(!build.contains("__bsmr_python_vcs"));
    }

    #[test]
    fn invariant_standard_metadata_creates_test_and_entry_point_targets() {
        let listing = PackageListing::testing_files(&[
            "pyproject.toml",
            "src/demo/__init__.py",
            "tests/test_demo.py",
        ]);
        let build = render_python_build_file(
            CellRelativePathBuf::unchecked_new(String::new()),
            "[project]\nname = 'demo'\nversion = '1'\nrequires-python = '>=3.12'\n[project.scripts]\ndemo = 'demo:main'\n[tool.bsmr.python]\ntest-command = ['tests/runtests.py', '--verbosity', '1']\n",
            &listing,
            &PythonRootFiles {
                config_files: Vec::new(),
                runtime_packages: Vec::new(),
                build_packages: Vec::new(),
                members: vec![PythonWorkspaceMember {
                    package: "packages/member".to_owned(),
                    target: "member".to_owned(),
                    dependencies: Vec::new(),
                    optional_dependencies: Default::default(),
                    build_requirements: Vec::new(),
                    dynamic_dependencies: false,
                    dynamic_optional_dependencies: false,
                }],
                test_locks: vec![
                    PythonTestLock {
                        environment: "__bsmr_python_test_environment".to_owned(),
                        file: "pylock.test.toml".to_owned(),
                        packages: package_fragments(&["pytest"]),
                        target: "test".to_owned(),
                    },
                    PythonTestLock {
                        environment: "__bsmr_python_test-all_environment".to_owned(),
                        file: "pylock.test-all.toml".to_owned(),
                        packages: package_fragments(&["pytest", "pytest-xdist"]),
                        target: "test-all".to_owned(),
                    },
                ],
                vcs: None,
            },
        )
        .unwrap();

        assert!(build.contains("package = \"pytest\""));
        assert!(build.contains("package = \"pytest-xdist\""));
        assert!(build.contains("python_test(\n    name = \"test\""));
        assert!(build.contains("python_test(\n    name = \"test-all\""));
        assert!(build.contains(
            "ty_check(\n    name = \"typecheck\",\n    environments = [\n        \"root//:__bsmr_python_test_environment\",\n    ],"
        ));
        assert!(build.contains(
            "    environments = [\n        \"root//:__bsmr_python_workspace_environment\",\n        \"root//:__bsmr_python_test_environment\",\n    ],"
        ));
        assert!(build.contains("entry = \"demo:main\""));
        assert!(build.contains("python_entry_point(\n    name = \"run\""));
        assert!(build.contains("\"//packages/member:member\""));
        assert_eq!(
            build
                .matches("    test_command = [\"tests/runtests.py\", \"--verbosity\", \"1\"],\n")
                .count(),
            2
        );
    }

    #[test]
    fn invariant_runtime_does_not_install_its_own_source_target() {
        let listing = PackageListing::testing_files(&["pyproject.toml", "src/demo/__init__.py"]);
        let build = render_python_build_file(
            CellRelativePathBuf::unchecked_new(String::new()),
            "[project]\nname = 'demo'\nversion = '1'\nrequires-python = '>=3.12'\n[project.scripts]\ndemo = 'demo:main'\n",
            &listing,
            &PythonRootFiles::default(),
        )
        .unwrap();
        let entry = build
            .split_once("python_entry_point(\n")
            .unwrap()
            .1
            .split_once("\n)\n")
            .unwrap()
            .0;

        assert!(entry.contains("root//:__bsmr_python_environment"));
        assert!(!entry.contains("__bsmr_python_workspace_environment"));
    }

    #[test]
    fn invariant_test_commands_are_nonempty_argv() {
        let listing = PackageListing::testing_files(&["pyproject.toml"]);

        assert!(matches!(
            render_python_build_file(
                CellRelativePathBuf::unchecked_new(String::new()),
                "[project]\nname = 'demo'\nversion = '1'\nrequires-python = '>=3.12'\n[tool.bsmr.python]\ntest-command = []\n",
                &listing,
                &root_files_with_source_packages(&[]),
            ),
            Err(NativePythonBuildError::InvalidTestCommand)
        ));
    }

    #[test]
    fn invariant_identical_packages_are_shared_across_lock_profiles() {
        let listing = PackageListing::testing_files(&["pyproject.toml"]);
        let packages = package_fragments(&["attrs"]);
        let build = render_python_build_file(
            CellRelativePathBuf::unchecked_new(String::new()),
            "[project]\nname = 'demo'\nversion = '1'\nrequires-python = '>=3.12'\n",
            &listing,
            &PythonRootFiles {
                config_files: Vec::new(),
                runtime_packages: package_fragments(&["attrs"]),
                build_packages: Vec::new(),
                members: Vec::new(),
                test_locks: vec![PythonTestLock {
                    environment: "__bsmr_python_test_environment".to_owned(),
                    file: "pylock.test.toml".to_owned(),
                    packages,
                    target: "test".to_owned(),
                }],
                vcs: None,
            },
        )
        .unwrap();

        assert_eq!(build.matches("python_locked_package(\n").count(), 1);
        assert_eq!(build.matches("    package = \"attrs\",\n").count(), 1);
    }

    #[test]
    fn invariant_named_test_locks_map_to_stable_target_names() {
        let listing = PackageListing::testing_files(&[
            "pylock.toml",
            "pylock.test.toml",
            "pylock.test-all.toml",
            "pylock.docs.toml",
            "packages/member/pylock.test-member.toml",
        ]);

        let locks = python_test_locks(&listing).unwrap();

        assert_eq!(locks.len(), 2);
        assert_eq!(locks[0].target, "test");
        assert_eq!(locks[0].environment, "__bsmr_python_test_environment");
        assert_eq!(locks[1].target, "test-all");
        assert_eq!(locks[1].environment, "__bsmr_python_test-all_environment");
    }

    #[test]
    fn invariant_test_profile_names_are_portable_target_labels() {
        let listing = PackageListing::testing_files(&["pylock.test-GPU.toml"]);

        assert!(matches!(
            python_test_locks(&listing),
            Err(NativePythonBuildError::InvalidTestLockName(_))
        ));
    }

    #[test]
    fn invariant_virtual_environments_never_become_workspace_members() {
        let listing = PackageListing::testing_files(&[
            "pyproject.toml",
            "packages/member/pyproject.toml",
            ".arbitrary-name/pyvenv.cfg",
            ".arbitrary-name/lib/python3.14/site-packages/demo/pyproject.toml",
        ]);

        assert_eq!(
            python_workspace_manifest_paths(
                "[tool.uv.workspace]\nmembers = ['packages/*', '.arbitrary-name/*']\n",
                &listing,
            )
            .unwrap(),
            vec!["packages/member/pyproject.toml"]
        );
    }

    #[test]
    fn invariant_uv_workspace_membership_excludes_incidental_projects() {
        let listing = PackageListing::testing_files(&[
            "pyproject.toml",
            "apps/api/pyproject.toml",
            "apps/private/pyproject.toml",
            "packages/typescript/tests/pyproject.toml",
        ]);

        assert_eq!(
            python_workspace_manifest_paths(
                "[tool.uv.workspace]\nmembers = ['apps/*']\nexclude = ['apps/private']\n",
                &listing,
            )
            .unwrap(),
            vec!["apps/api/pyproject.toml"]
        );
    }

    #[test]
    fn invariant_workspace_closure_is_transitive_and_package_granular() {
        let members = [
            python_workspace_member(
                "packages/auth".to_owned(),
                "[project]\nname = 'dedalus-auth'\nrequires-python = '>=3.14'\ndependencies = ['dedalus-io[fast]>=1; sys_platform == \"darwin\"']\n",
            )
            .unwrap()
            .unwrap(),
            python_workspace_member(
                "packages/io".to_owned(),
                "[project]\nname = 'dedalus-io'\nrequires-python = '>=3.14'\n",
            )
            .unwrap()
            .unwrap(),
            python_workspace_member(
                "tools/cind".to_owned(),
                "[project]\nname = 'cind'\nrequires-python = '>=3.14'\n",
            )
            .unwrap()
            .unwrap(),
        ];

        let closure = python_workspace_closure(
            "[project]\nname = 'api'\nrequires-python = '>=3.14'\ndependencies = ['dedalus-auth']\n",
            &members,
        )
        .unwrap();

        assert_eq!(
            closure
                .iter()
                .map(|member| member.target.as_str())
                .collect::<Vec<_>>(),
            ["dedalus-auth", "dedalus-io"]
        );
    }

    #[test]
    fn invariant_hatch_dynamic_metadata_selects_only_requested_workspace_extras() {
        let manifests = [
            (
                "pydantic_ai_slim",
                "[project]\nname = 'pydantic-ai-slim'\nrequires-python = '>=3.14'\ndynamic = ['dependencies', 'optional-dependencies']\n[tool.hatch.metadata.hooks.uv-dynamic-versioning]\ndependencies = ['pydantic-graph']\n[tool.hatch.metadata.hooks.uv-dynamic-versioning.optional-dependencies]\nevals = ['pydantic-evals']\n",
            ),
            (
                "pydantic_evals",
                "[project]\nname = 'pydantic-evals'\nrequires-python = '>=3.14'\n",
            ),
            (
                "pydantic_graph",
                "[project]\nname = 'pydantic-graph'\nrequires-python = '>=3.14'\n",
            ),
            (
                "examples",
                "[project]\nname = 'pydantic-ai-examples'\nrequires-python = '>=3.14'\n",
            ),
            (
                "clai",
                "[project]\nname = 'clai'\nrequires-python = '>=3.14'\n",
            ),
        ];
        let members = manifests
            .into_iter()
            .map(|(package, manifest)| {
                python_workspace_member(package.to_owned(), manifest)
                    .unwrap()
                    .unwrap()
            })
            .collect::<Vec<_>>();

        let closure = python_workspace_closure(
            "[project]\nname = 'pydantic-ai'\nrequires-python = '>=3.14'\ndynamic = ['dependencies']\n[tool.hatch.metadata.hooks.uv-dynamic-versioning]\ndependencies = ['pydantic-ai-slim[evals,openai]']\n",
            &members,
        )
        .unwrap();

        assert_eq!(
            closure
                .iter()
                .map(|member| member.target.as_str())
                .collect::<Vec<_>>(),
            ["pydantic-ai-slim", "pydantic-evals", "pydantic-graph"]
        );
    }

    #[test]
    fn invariant_unknown_dynamic_dependencies_fail_before_graph_analysis() {
        let error = python_workspace_closure(
            "[project]\nname = 'opaque'\nrequires-python = '>=3.14'\ndynamic = ['dependencies']\n",
            &[],
        )
        .unwrap_err();

        assert!(matches!(
            error,
            NativePythonBuildError::UnsupportedDynamicDependencies(project)
                if project == "opaque"
        ));
    }

    #[test]
    fn invariant_build_lock_covers_pep517_and_uv_compatibility_requirements() {
        let manifest = "[project]\nname = 'api'\nrequires-python = '>=3.14'\n[build-system]\nrequires = ['hatchling>=1.28']\nbuild-backend = 'hatchling.build'\n[tool.uv.extra-build-dependencies]\npyroaring = ['cython>=3']\n";
        let lock = wheel_lock(&[("cython", "3.2.9"), ("hatchling", "1.32.0")]);

        validate_python_build_requirements(manifest, &[], &lock, &lock).unwrap();
    }

    #[test]
    fn invariant_missing_build_requirement_fails_before_rendering() {
        let lock = wheel_lock(&[("hatchling", "1.32.0")]);
        let error = validate_python_build_requirements(
            "[project]\nname = 'api'\nrequires-python = '>=3.14'\n[tool.uv.extra-build-dependencies]\npyroaring = ['cython>=3']\n",
            &[],
            &lock,
            &lock,
        )
        .unwrap_err();

        assert!(matches!(
            error,
            NativePythonBuildError::MissingBuildRequirement(requirement)
                if requirement == "cython"
        ));
    }

    #[test]
    fn invariant_match_runtime_build_dependencies_have_identical_versions() {
        let manifest = "[project]\nname = 'demo'\nrequires-python = '>=3.14'\n[tool.uv.extra-build-dependencies]\nflash-attn = [{ requirement = 'torch', match-runtime = true }]\n";
        let runtime = wheel_lock(&[("torch", "2.10.0")]);
        let build = wheel_lock(&[("torch", "2.9.0")]);

        let error =
            validate_python_build_requirements(manifest, &[], &runtime, &build).unwrap_err();

        assert!(matches!(
            error,
            NativePythonBuildError::BuildRequirementVersionMismatch {
                package,
                runtime,
                build,
            } if package == "torch" && runtime == ["2.10.0"] && build == ["2.9.0"]
        ));
    }

    #[test]
    fn invariant_nested_project_does_not_install_its_own_source_target() {
        let listing = PackageListing::testing_files(&["pyproject.toml", "pkg/__init__.py"]);
        let root_files = PythonRootFiles {
            config_files: vec![".ruff.toml".to_owned(), "pyproject.toml".to_owned()],
            ..PythonRootFiles::default()
        };
        let build = render_python_build_file(
            CellRelativePathBuf::unchecked_new("packages/api".to_owned()),
            "[project]\nname = 'api'\nrequires-python = '>=3.12'\n",
            &listing,
            &root_files,
        )
        .unwrap();

        assert!(build.contains("\"root//:__bsmr_python_environment\""));
        assert!(!build.contains("python_wheel_environment("));
        assert!(!build.contains("\":api\""));
        assert!(build.contains("python = \"root//:__bsmr_python_distribution\""));
        assert!(build.contains("distribution = \"api\""));
        assert!(!build.contains("uv = \"root//:__bsmr_uv_distribution\""));
        assert!(!build.contains("python_environment("));
        assert!(build.contains("\"packages/api/pkg/__init__.py\""));
        assert_eq!(
            build
                .matches("\".ruff.toml\": \"root//:__bsmr_python_config__ruff_toml\"")
                .count(),
            2
        );
        assert_eq!(
            build
                .matches("\"pyproject.toml\": \"root//:__bsmr_python_config_pyproject_toml\"")
                .count(),
            2
        );
    }

    #[test]
    fn invariant_only_native_root_analysis_configs_propagate() {
        let listing = PackageListing::testing_files(&[
            ".ruff.toml",
            "pyproject.toml",
            "ruff.toml",
            "ty.toml",
            "unrelated.toml",
        ]);

        let config_files = python_root_config_files(&listing);
        assert_eq!(
            config_files,
            [".ruff.toml", "pyproject.toml", "ruff.toml", "ty.toml"]
        );
        let build = render_python_build_file(
            CellRelativePathBuf::unchecked_new(String::new()),
            "[project]\nname = 'demo'\nrequires-python = '>=3.14'\n",
            &listing,
            &PythonRootFiles {
                config_files,
                ..PythonRootFiles::default()
            },
        )
        .unwrap();
        for file in [".ruff.toml", "pyproject.toml", "ruff.toml", "ty.toml"] {
            let target = format!("__bsmr_python_config_{}", file.replace('.', "_"));
            assert!(build.contains(&format!(
                "export_file(\n    name = {target:?},\n    src = {file:?},"
            )));
        }
        assert!(!build.contains("src = \"unrelated.toml\","));
    }

    #[test]
    fn invariant_project_metadata_requires_an_interpreter_contract() {
        let listing = PackageListing::testing_files(&["pyproject.toml"]);

        assert!(matches!(
            render_python_build_file(
                CellRelativePathBuf::unchecked_new(String::new()),
                "[project]\nname = 'demo'\n",
                &listing,
                &PythonRootFiles::default(),
            ),
            Err(NativePythonBuildError::MissingProjectMetadata)
        ));
    }
}
