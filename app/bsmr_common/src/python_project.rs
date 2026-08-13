//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Renders private Python rules from standard project metadata.

use std::collections::BTreeMap;

use bsmr_core::cells::paths::CellRelativePathBuf;
use bsmr_core::package::package_relative_path::PackageRelativePath;
use serde::Deserialize;

use crate::package_listing::listing::PackageListing;

mod render;

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
    /// Test profile names become target labels and therefore use one portable spelling.
    #[error(
        "Python test lock `{0}` must use pylock.test.toml or pylock.test-<name>.toml with lowercase ASCII letters, digits, and hyphens"
    )]
    InvalidTestLockName(String),
    /// Formatting internal Starlark into a string should be infallible.
    #[error("failed to render native Python build graph")]
    Render(std::fmt::Error),
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct Manifest {
    project: Option<Project>,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct Project {
    name: String,
    #[serde(default)]
    dynamic: Vec<String>,
    requires_python: Option<String>,
    #[serde(default)]
    scripts: BTreeMap<String, String>,
}

/// Git metadata files that make dynamic project versions explicit action inputs.
pub struct PythonVcsFiles {
    /// Whether dynamic versioning may resolve packed rather than loose refs.
    pub packed_refs: bool,
    /// Whether Git must preserve a shallow repository boundary during version resolution.
    pub shallow: bool,
}

/// Root-only Python files that alter which private graph nodes are available.
#[derive(Default)]
pub struct PythonRootFiles {
    /// First-party wheels overlaid into each runnable environment.
    pub members: Vec<PythonWorkspaceMember>,
    /// Named dependency closures that become generated test targets.
    pub test_locks: Vec<PythonTestLock>,
    /// Declared Git inputs for projects whose version is computed dynamically.
    pub vcs: Option<PythonVcsFiles>,
}

/// One named PEP 751 installation set exposed as a native test target.
pub struct PythonTestLock {
    /// Private target that materializes the lock into a CAS directory.
    pub environment: String,
    /// Root-relative PEP 751 lock consumed by the environment action.
    pub file: String,
    /// Public test label derived from the lock's portable profile name.
    pub target: String,
}

/// One first-party distribution materialized into shared runtime environments.
pub struct PythonWorkspaceMember {
    /// Root-relative package containing the member project.
    pub package: String,
    /// Canonical wheel target generated from the member's project name.
    pub target: String,
}

/// Returns the canonical target name when a manifest declares an installable project.
#[must_use = "the generated target name determines whether this manifest joins the build graph"]
pub fn python_project_name(manifest: &str) -> Result<Option<String>, NativePythonBuildError> {
    let manifest =
        toml::from_str::<Manifest>(manifest).map_err(NativePythonBuildError::InvalidManifest)?;
    manifest
        .project
        .map(|project| {
            project
                .requires_python
                .as_ref()
                .ok_or(NativePythonBuildError::MissingProjectMetadata)?;
            Ok(normalize_project_name(&project.name))
        })
        .transpose()
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
pub fn python_workspace_manifest_paths(listing: &PackageListing) -> Vec<&str> {
    let virtual_environments = virtual_environment_roots(listing);
    listing
        .files()
        .files()
        .map(PackageRelativePath::as_str)
        .filter(|path| *path != "pyproject.toml" && path.ends_with("/pyproject.toml"))
        .filter(|path| !is_generated_path(path, &virtual_environments))
        .collect()
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
    }) || (path.starts_with("pylock.") && path.ends_with(".toml"))
        || path == "pylock.toml"
        || path == "uv.lock"
        || path.split('/').any(|component| {
            matches!(
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
            ) || component.ends_with(".egg-info")
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
            Some(".gitignore" | "pyproject.toml" | "ruff.toml" | "ty.toml")
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
    use bsmr_core::cells::paths::CellRelativePathBuf;

    use super::NativePythonBuildError;
    use super::PythonRootFiles;
    use super::PythonTestLock;
    use super::PythonVcsFiles;
    use super::PythonWorkspaceMember;
    use super::python_project_name;
    use super::python_test_locks;
    use super::python_workspace_manifest_paths;
    use super::render_python_build_file;
    use crate::package_listing::listing::PackageListing;
    use crate::package_listing::listing::testing::PackageListingExt;

    #[test]
    fn invariant_tool_only_manifests_are_not_workspace_projects() {
        let project = python_project_name("[tool.ruff]\nline-length = 88\n").unwrap();

        assert_eq!(project, None);
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
            "pyproject.toml",
            "pylock.toml",
            "src/demo/__init__.py",
            "src/demo/__pycache__/ignored.pyc",
            "examples/pyproject.toml",
            "README.md",
            ".venv/ignored.py",
            ".custom-env/pyvenv.cfg",
            ".custom-env/lib/python3.14/site-packages/demo/pyproject.toml",
            ".custom-env/lib/python3.14/site-packages/demo/__init__.py",
        ]);
        let build = render_python_build_file(
            CellRelativePathBuf::unchecked_new(String::new()),
            "[project]\nname = 'Demo_Project'\nrequires-python = '>=3.12'\n",
            &listing,
            &PythonRootFiles::default(),
        )
        .unwrap();

        assert!(build.contains("name = \"demo-project\""));
        assert!(build.contains("name = \"lint\""));
        assert!(build.contains("name = \"typecheck\""));
        assert!(build.contains("python_environment("));
        assert!(build.contains("python_wheel_environment("));
        assert_eq!(build.matches("    wheels = [").count(), 1);
        assert!(build.contains("name = \"__bsmr_python_build_environment\""));
        assert!(build.contains("lock = \"pylock.build.toml\""));
        assert_eq!(
            build
                .matches("build_environment = \":__bsmr_python_build_environment\"")
                .count(),
            1
        );
        assert!(build.contains("\"src/demo/__init__.py\""));
        assert!(!build.contains(".venv/ignored.py"));
        assert!(!build.contains(".custom-env"));
        assert!(!build.contains("__pycache__"));
        assert_eq!(build.matches("\"examples/pyproject.toml\"").count(), 4);
        assert_eq!(build.matches("\"README.md\"").count(), 2);
    }

    #[test]
    fn invariant_each_action_depends_only_on_the_tools_it_consumes() {
        let listing = PackageListing::testing_files(&["pyproject.toml", "src/demo/__init__.py"]);
        let build = render_python_build_file(
            CellRelativePathBuf::unchecked_new(String::new()),
            "[project]\nname = 'demo'\nrequires-python = '>=3.12'\n",
            &listing,
            &PythonRootFiles::default(),
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
        assert!(wheel.contains("__bsmr_uv_distribution"));
        assert!(wheel.contains("environment = \"root//:__bsmr_python_build_environment\""));
        assert!(!wheel.contains("__bsmr_ruff_distribution"));
        assert!(!wheel.contains("__bsmr_ty_distribution"));

        let lint = body("ruff_check");
        assert!(lint.contains("__bsmr_ruff_distribution"));
        assert!(!lint.contains("environment ="));
        assert!(!lint.contains("__bsmr_uv_distribution"));
        assert!(!lint.contains("__bsmr_ty_distribution"));

        let typecheck = body("ty_check");
        assert!(typecheck.contains("environment ="));
        assert!(typecheck.contains("__bsmr_ty_distribution"));
        assert!(!typecheck.contains("__bsmr_uv_distribution"));
        assert!(!typecheck.contains("__bsmr_ruff_distribution"));
    }

    #[test]
    fn invariant_dynamic_versions_depend_on_declared_git_state() {
        let listing = PackageListing::testing_files(&["pyproject.toml", "src/demo/__init__.py"]);
        let build = render_python_build_file(
            CellRelativePathBuf::unchecked_new(String::new()),
            "[project]\nname = 'demo'\ndynamic = ['version']\nrequires-python = '>=3.12'\n",
            &listing,
            &PythonRootFiles {
                members: Vec::new(),
                test_locks: Vec::new(),
                vcs: Some(PythonVcsFiles {
                    packed_refs: true,
                    shallow: false,
                }),
            },
        )
        .unwrap();

        assert!(build.contains("python_vcs("));
        assert!(build.contains("\"packed-refs\": \".git/packed-refs\""));
        assert!(!build.contains("\"index\": \".git/index\""));
        assert!(!build.contains("\"shallow\": \".git/shallow\""));
        assert!(build.contains("vcs = \"root//:__bsmr_python_vcs\""));
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
            "[project]\nname = 'demo'\nversion = '1'\nrequires-python = '>=3.12'\n[project.scripts]\ndemo = 'demo:main'\n",
            &listing,
            &PythonRootFiles {
                members: vec![PythonWorkspaceMember {
                    package: "packages/member".to_owned(),
                    target: "member".to_owned(),
                }],
                test_locks: vec![
                    PythonTestLock {
                        environment: "__bsmr_python_test_environment".to_owned(),
                        file: "pylock.test.toml".to_owned(),
                        target: "test".to_owned(),
                    },
                    PythonTestLock {
                        environment: "__bsmr_python_test-all_environment".to_owned(),
                        file: "pylock.test-all.toml".to_owned(),
                        target: "test-all".to_owned(),
                    },
                ],
                vcs: None,
            },
        )
        .unwrap();

        assert!(build.contains("lock = \"pylock.test.toml\""));
        assert!(build.contains("lock = \"pylock.test-all.toml\""));
        assert!(build.contains("python_test(\n    name = \"test\""));
        assert!(build.contains("python_test(\n    name = \"test-all\""));
        assert!(build.contains(
            "    environments = [\n        \"root//:__bsmr_python_workspace_environment\",\n        \"root//:__bsmr_python_test_environment\",\n    ],"
        ));
        assert!(build.contains("entry = \"demo:main\""));
        assert!(build.contains("python_entry_point(\n    name = \"run\""));
        assert!(build.contains("\"//packages/member:member\""));
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
            python_workspace_manifest_paths(&listing),
            vec!["packages/member/pyproject.toml"]
        );
    }

    #[test]
    fn invariant_nested_project_reuses_the_root_environment() {
        let listing = PackageListing::testing_files(&["pyproject.toml", "pkg/__init__.py"]);
        let build = render_python_build_file(
            CellRelativePathBuf::unchecked_new("packages/api".to_owned()),
            "[project]\nname = 'api'\nrequires-python = '>=3.12'\n",
            &listing,
            &PythonRootFiles::default(),
        )
        .unwrap();

        assert!(build.contains("environment = \"root//:__bsmr_python_environment\""));
        assert!(!build.contains("python_environment("));
        assert!(build.contains("\"packages/api/pkg/__init__.py\""));
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
