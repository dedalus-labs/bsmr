//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Renders internal TypeScript rules from an authoritative pnpm workspace graph.

use std::fmt::Write;

use bsmr_core::cells::paths::CellRelativePathBuf;
use bsmr_core::package::package_relative_path::PackageRelativePath;

use super::WorkspaceGraph;
use super::toolchain::TOOLCHAIN_TARGET;
use super::toolchain::render_toolchain;
use crate::package_listing::listing::PackageListing;

const SOURCES_TARGET: &str = "__bsmr_sources";
const INSTALL_TARGET: &str = "__bsmr_dependencies";

/// Returns whether the root declares pnpm's authoritative frozen input.
#[must_use]
pub fn is_native_pnpm_workspace(listing: &PackageListing) -> bool {
    listing
        .get_file(PackageRelativePath::unchecked_new("pnpm-lock.yaml"))
        .is_some()
}

/// A native TypeScript package cannot be lowered without satisfying these invariants.
#[derive(Debug, bsmr_error::Error)]
#[bsmr(tag = Input)]
pub enum NativeTypeScriptBuildError {
    /// The path selector's inferred target must be a valid, unambiguous package name.
    #[error(
        "package `{0}` has no usable path component or npm package name for its default target"
    )]
    MissingTargetName(CellRelativePathBuf),
    /// BSMR's private target namespace must remain unavailable to ecosystem packages.
    #[error("package `{0}` conflicts with BSMR's reserved target name `{1}`")]
    ReservedTargetName(CellRelativePathBuf, String),
    /// A source package needs both semantic and emission configuration.
    #[error("TypeScript package `{0}` must contain tsconfig.json and tsdown.config.ts")]
    MissingCompilerConfig(CellRelativePathBuf),
    /// The workspace root owns the one frozen install action.
    #[error("pnpm workspace root is missing pnpm-lock.yaml")]
    MissingLockfile,
    /// Native builds require exact root toolchain requirements.
    #[error("workspace package.json must declare engines.node and packageManager")]
    MissingToolchain,
    /// The Node requirement must use npm's semver grammar.
    #[error("invalid engines.node requirement `{requirement}`: {error}")]
    InvalidNodeRequirement { requirement: String, error: String },
    /// At least one cataloged Node runtime must satisfy the project requirement.
    #[error(
        "no cataloged Node runtime satisfies engines.node `{requirement}`; available: {available}"
    )]
    UnsupportedNodeRequirement {
        requirement: String,
        available: String,
    },
    /// Package-manager distributions are deliberately finite and digest-pinned.
    #[error("unsupported packageManager `{0}`; BSMR supports exact pnpm 10.30.3 and 11.20.0 pins")]
    UnsupportedPackageManager(String),
    /// Formatting internal Starlark into a string should be infallible.
    #[error("failed to render native TypeScript build graph")]
    Render(std::fmt::Error),
}

/// Renders the private Starlark bridge for an authoritative pnpm package.
///
/// Returns `None` when `package_root` is outside the workspace graph. This lets
/// polyglot repositories retain incidental `package.json` files without
/// turning them into native TypeScript packages.
pub fn render_typescript_build_file(
    graph: &WorkspaceGraph,
    package_root: CellRelativePathBuf,
    listing: &PackageListing,
) -> Result<Option<String>, NativeTypeScriptBuildError> {
    let Some(package_name) = graph.package_name_at_root(&package_root) else {
        return Ok(None);
    };
    let target_name = default_target_name(&package_root, package_name)
        .ok_or_else(|| NativeTypeScriptBuildError::MissingTargetName(package_root.clone()))?;
    let target_name = target_name.to_owned();
    if matches!(
        target_name.as_str(),
        SOURCES_TARGET | INSTALL_TARGET | "typecheck"
    ) {
        return Err(NativeTypeScriptBuildError::ReservedTargetName(
            package_root,
            target_name.clone(),
        ));
    }

    let package_files = package_files(graph, &package_root, listing);
    let has_tsconfig = package_files.contains(&"tsconfig.json");
    let has_tsdown = package_files.contains(&"tsdown.config.ts");
    if has_tsdown && !has_tsconfig {
        return Err(NativeTypeScriptBuildError::MissingCompilerConfig(
            package_root,
        ));
    }

    let mut output = String::new();
    if package_root.is_empty() {
        render_install(graph, listing, &mut output)?;
    }

    writeln!(
        output,
        "load(\"@prelude//typescript:defs.bzl\", \"typescript_library\", \"typescript_sources\", \"typescript_typecheck\")"
    )
    .map_err(NativeTypeScriptBuildError::Render)?;
    writeln!(output).map_err(NativeTypeScriptBuildError::Render)?;
    writeln!(output, "typescript_sources(").map_err(NativeTypeScriptBuildError::Render)?;
    writeln!(output, "    name = {SOURCES_TARGET:?},")
        .map_err(NativeTypeScriptBuildError::Render)?;
    writeln!(output, "    srcs = {{").map_err(NativeTypeScriptBuildError::Render)?;
    for file in &package_files {
        let workspace_path = workspace_path(&package_root, file);
        writeln!(output, "        {workspace_path:?}: {file:?},")
            .map_err(NativeTypeScriptBuildError::Render)?;
    }
    writeln!(output, "    }},").map_err(NativeTypeScriptBuildError::Render)?;
    writeln!(output, "    deps = [").map_err(NativeTypeScriptBuildError::Render)?;
    let project = graph
        .package(package_name)
        .expect("package indexed by root");
    for dependency in project.dependencies() {
        let root = graph
            .package(dependency)
            .expect("workspace dependency was validated")
            .root();
        writeln!(
            output,
            "        {:?},",
            format!("root//{}:{SOURCES_TARGET}", root)
        )
        .map_err(NativeTypeScriptBuildError::Render)?;
    }
    writeln!(output, "    ],").map_err(NativeTypeScriptBuildError::Render)?;
    writeln!(output, "    visibility = [\"PUBLIC\"],")
        .map_err(NativeTypeScriptBuildError::Render)?;
    writeln!(output, ")\n").map_err(NativeTypeScriptBuildError::Render)?;

    if !has_tsdown {
        return Ok(Some(output));
    }

    let package_root_arg = if package_root.is_empty() {
        "."
    } else {
        package_root.as_str()
    };
    for (rule, name) in [
        ("typescript_library", target_name.as_str()),
        ("typescript_typecheck", "typecheck"),
    ] {
        writeln!(output, "{rule}(").map_err(NativeTypeScriptBuildError::Render)?;
        writeln!(output, "    name = {name:?},").map_err(NativeTypeScriptBuildError::Render)?;
        writeln!(output, "    install = \"root//:{INSTALL_TARGET}\",")
            .map_err(NativeTypeScriptBuildError::Render)?;
        writeln!(output, "    package_root = {package_root_arg:?},")
            .map_err(NativeTypeScriptBuildError::Render)?;
        writeln!(output, "    sources = \":{SOURCES_TARGET}\",")
            .map_err(NativeTypeScriptBuildError::Render)?;
        writeln!(output, ")\n").map_err(NativeTypeScriptBuildError::Render)?;
    }
    Ok(Some(output))
}

/// Returns the target inferred by BSMR's relaxed path selector.
fn default_target_name<'a>(
    root: &'a CellRelativePathBuf,
    package_name: &'a str,
) -> Option<&'a str> {
    root.file_name().map(|name| name.as_str()).or_else(|| {
        package_name
            .rsplit('/')
            .next()
            .filter(|name| !name.is_empty())
    })
}

/// Selects this package's files while excluding nested workspace packages.
fn package_files<'a>(
    graph: &WorkspaceGraph,
    package_root: &CellRelativePathBuf,
    listing: &'a PackageListing,
) -> Vec<&'a str> {
    listing
        .files()
        .files()
        .map(PackageRelativePath::as_str)
        .filter(|file| {
            !graph.packages.values().any(|project| {
                if project.root == *package_root {
                    return false;
                }
                let nested_root = if package_root.is_empty() {
                    Some(project.root.as_str())
                } else {
                    project
                        .root
                        .as_str()
                        .strip_prefix(package_root.as_str())
                        .and_then(|suffix| suffix.strip_prefix('/'))
                };
                nested_root.is_some_and(|root| {
                    !root.is_empty()
                        && (*file == root
                            || file
                                .strip_prefix(root)
                                .is_some_and(|suffix| suffix.starts_with('/')))
                })
            })
        })
        .filter(|file| !is_generated_path(file))
        .collect()
}

/// Excludes mutable dependency and compiler output trees from action inputs.
fn is_generated_path(path: &str) -> bool {
    [".bsmr", ".git", "dist", "node_modules"]
        .into_iter()
        .any(|root| path == root || path.starts_with(&format!("{root}/")))
}

/// Joins a package-local file to its workspace-relative path.
fn workspace_path(package_root: &CellRelativePathBuf, file: &str) -> String {
    if package_root.is_empty() {
        file.to_owned()
    } else {
        format!("{package_root}/{file}")
    }
}

/// Renders the workspace's single frozen pnpm installation target.
fn render_install(
    graph: &WorkspaceGraph,
    listing: &PackageListing,
    output: &mut String,
) -> Result<(), NativeTypeScriptBuildError> {
    let files = listing
        .files()
        .files()
        .map(PackageRelativePath::as_str)
        .collect::<Vec<_>>();
    if !files.contains(&"pnpm-lock.yaml") {
        return Err(NativeTypeScriptBuildError::MissingLockfile);
    }
    render_toolchain(graph, output)?;
    writeln!(
        output,
        "load(\"@prelude//toolchains/pnpm:defs.bzl\", \"pnpm_install\")\n"
    )
    .map_err(NativeTypeScriptBuildError::Render)?;
    writeln!(output, "pnpm_install(").map_err(NativeTypeScriptBuildError::Render)?;
    writeln!(output, "    name = {INSTALL_TARGET:?},")
        .map_err(NativeTypeScriptBuildError::Render)?;
    writeln!(output, "    package_json = \"package.json\",")
        .map_err(NativeTypeScriptBuildError::Render)?;
    writeln!(output, "    pnpm_lock = \"pnpm-lock.yaml\",")
        .map_err(NativeTypeScriptBuildError::Render)?;
    writeln!(output, "    toolchain = \":{TOOLCHAIN_TARGET}\",")
        .map_err(NativeTypeScriptBuildError::Render)?;
    writeln!(output, "    srcs = {{").map_err(NativeTypeScriptBuildError::Render)?;
    for file in files.into_iter().filter(|file| is_install_input(file)) {
        writeln!(output, "        {file:?}: {file:?},")
            .map_err(NativeTypeScriptBuildError::Render)?;
    }
    writeln!(output, "    }},").map_err(NativeTypeScriptBuildError::Render)?;
    writeln!(output, "    visibility = [\"PUBLIC\"],")
        .map_err(NativeTypeScriptBuildError::Render)?;
    writeln!(output, ")\n").map_err(NativeTypeScriptBuildError::Render)?;
    Ok(())
}

/// Selects only files pnpm may read with lifecycle scripts disabled.
fn is_install_input(path: &str) -> bool {
    path == "pnpm-workspace.yaml"
        || path == ".npmrc"
        || path == ".pnpmfile.cjs"
        || path.ends_with("/package.json")
        || path.starts_with("patches/")
}

#[cfg(test)]
mod tests {
    use bsmr_core::cells::paths::CellRelativePathBuf;

    use super::is_native_pnpm_workspace;
    use super::render_typescript_build_file;
    use crate::package_listing::listing::PackageListing;
    use crate::package_listing::listing::testing::PackageListingExt;
    use crate::pnpm_workspace::WorkspaceGraph;
    use crate::pnpm_workspace::WorkspacePackage;

    /// Parses one workspace package fixture.
    fn package(root: &str, manifest: &str) -> WorkspacePackage {
        WorkspacePackage::parse(
            CellRelativePathBuf::try_from(root.to_owned()).unwrap(),
            manifest,
        )
        .unwrap()
    }

    #[test]
    fn invariant_incidental_package_json_does_not_activate_pnpm() {
        let listing = PackageListing::testing_files(&["package.json", "pyproject.toml"]);

        assert!(!is_native_pnpm_workspace(&listing));
    }

    #[test]
    fn invariant_package_json_outside_the_workspace_is_not_a_typescript_package() {
        let graph =
            WorkspaceGraph::build([package("packages/api", r#"{"name":"@acme/api"}"#)]).unwrap();
        let listing = PackageListing::testing_files(&["package.json"]);

        let build = render_typescript_build_file(
            &graph,
            CellRelativePathBuf::unchecked_new(".github/actions/src".to_owned()),
            &listing,
        )
        .unwrap();

        assert_eq!(build, None);
    }

    #[test]
    fn invariant_native_package_uses_path_api_and_exact_dependency_sources() {
        let graph = WorkspaceGraph::build([
            package(
                "apps/api",
                r#"{"name":"@acme/api","dependencies":{"@acme/core":"workspace:*"}}"#,
            ),
            package("packages/core", r#"{"name":"@acme/core"}"#),
        ])
        .unwrap();
        let listing = PackageListing::testing_files(&[
            "package.json",
            "src/index.ts",
            "tsconfig.json",
            "tsdown.config.ts",
        ]);

        let build = render_typescript_build_file(
            &graph,
            CellRelativePathBuf::unchecked_new("apps/api".to_owned()),
            &listing,
        )
        .unwrap()
        .unwrap();

        assert!(build.contains("name = \"api\""));
        assert!(build.contains("\"root//packages/core:__bsmr_sources\""));
        assert!(build.contains("\"apps/api/src/index.ts\": \"src/index.ts\""));
        assert!(!build.contains(":lib"));
    }

    #[test]
    fn invariant_config_only_workspace_packages_export_their_files() {
        let graph = WorkspaceGraph::build([package(
            "packages/tsconfig",
            r#"{"name":"@acme/tsconfig","main":"base.json"}"#,
        )])
        .unwrap();
        let listing = PackageListing::testing_files(&["base.json", "package.json"]);

        let build = render_typescript_build_file(
            &graph,
            CellRelativePathBuf::unchecked_new("packages/tsconfig".to_owned()),
            &listing,
        )
        .unwrap()
        .unwrap();

        assert!(build.contains("typescript_sources("));
        assert!(build.contains("\"packages/tsconfig/base.json\": \"base.json\""));
        assert!(!build.contains("typescript_library("));
        assert!(!build.contains("typescript_typecheck("));
    }

    #[test]
    fn invariant_nested_workspace_package_is_not_an_input_of_its_parent() {
        let graph = WorkspaceGraph::build([
            package("apps/api", r#"{"name":"@acme/api"}"#),
            package("apps/api/plugins/auth", r#"{"name":"@acme/auth-plugin"}"#),
        ])
        .unwrap();
        let listing = PackageListing::testing_files(&[
            "package.json",
            "src/index.ts",
            "tsconfig.json",
            "tsdown.config.ts",
            "plugins/auth/package.json",
            "plugins/auth/src/index.ts",
        ]);

        let build = render_typescript_build_file(
            &graph,
            CellRelativePathBuf::unchecked_new("apps/api".to_owned()),
            &listing,
        )
        .unwrap()
        .unwrap();

        assert!(!build.contains("plugins/auth/package.json"));
        assert!(!build.contains("plugins/auth/src/index.ts"));
    }

    #[test]
    fn invariant_workspace_install_uses_manifests_not_source_tree() {
        let graph = WorkspaceGraph::build([
            package(
                "",
                r#"{
                    "name":"@acme/root",
                    "engines":{"node":"26.5.1"},
                    "packageManager":"pnpm@11.20.0+sha512.9a6f330a95b66446ea088faf1521405a8a01f07fde7124cc9958dfed52d4bb436737e65b08f85f37b46fcba375092558ac51262b816844b22f63406ed166bfee"
                }"#,
            ),
            package("packages/api", r#"{"name":"@acme/api"}"#),
        ])
        .unwrap();
        let listing = PackageListing::testing_files(&[
            "package.json",
            "pnpm-lock.yaml",
            "pnpm-workspace.yaml",
            "packages/api/package.json",
            "packages/api/src/index.ts",
        ]);

        let build = render_typescript_build_file(
            &graph,
            CellRelativePathBuf::unchecked_new(String::new()),
            &listing,
        )
        .unwrap()
        .unwrap();

        assert!(build.contains("name = \"__bsmr_dependencies\""));
        assert!(build.contains("name = \"__bsmr_pnpm_toolchain\""));
        assert!(build.contains("node-v26.5.1-linux-arm64.tar.gz"));
        assert!(build.contains("pnpm-11.20.0.tgz"));
        assert!(build.contains("\"packages/api/package.json\""));
        assert!(!build.contains("packages/api/src/index.ts"));
    }

    /// Tooling scripts do not turn a workspace root into an emitted package.
    #[test]
    fn invariant_workspace_root_without_tsdown_is_not_an_emitted_package() {
        let graph = WorkspaceGraph::build([package(
            "",
            r#"{"name":"@acme/root","engines":{"node":">=26"},"packageManager":"pnpm@11.20.0+sha512.9a6f330a95b66446ea088faf1521405a8a01f07fde7124cc9958dfed52d4bb436737e65b08f85f37b46fcba375092558ac51262b816844b22f63406ed166bfee"}"#,
        )])
        .unwrap();
        let listing = PackageListing::testing_files(&[
            "Cargo.toml",
            "ci/action.ts",
            "package.json",
            "pnpm-lock.yaml",
            "tsconfig.json",
            "types.d.ts",
        ]);

        let build = render_typescript_build_file(
            &graph,
            CellRelativePathBuf::unchecked_new(String::new()),
            &listing,
        )
        .unwrap()
        .unwrap();

        assert!(build.contains("pnpm_install("));
        assert!(!build.contains("typescript_library("));
    }

    #[test]
    fn invariant_pnpm_10_accepts_a_compatible_node_range() {
        let graph = WorkspaceGraph::build([package(
            "",
            r#"{
                "name":"@acme/root",
                "engines":{"node":">=24.0.0"},
                "packageManager":"pnpm@10.30.3+sha512.c961d1e0a2d8e354ecaa5166b822516668b7f44cb5bd95122d590dd81922f606f5473b6d23ec4a5be05e7fcd18e8488d47d978bbe981872f1145d06e9a740017"
            }"#,
        )])
        .unwrap();
        let listing = PackageListing::testing_files(&[
            "package.json",
            "pnpm-lock.yaml",
            "pnpm-workspace.yaml",
        ]);

        let build = render_typescript_build_file(
            &graph,
            CellRelativePathBuf::unchecked_new(String::new()),
            &listing,
        )
        .unwrap()
        .unwrap();

        assert!(build.contains("pnpm-10.30.3.tgz"));
        assert!(build.contains("node_requirement = \">=24.0.0\""));
    }

    #[test]
    fn invariant_native_toolchain_rejects_unsupported_versions() {
        let graph = WorkspaceGraph::build([package(
            "",
            r#"{
                "name":"@acme/root",
                "engines":{"node":">=27.0.0"},
                "packageManager":"pnpm@12.0.0"
            }"#,
        )])
        .unwrap();
        let listing = PackageListing::testing_files(&["package.json", "pnpm-lock.yaml"]);

        let error = render_typescript_build_file(
            &graph,
            CellRelativePathBuf::unchecked_new(String::new()),
            &listing,
        )
        .unwrap_err();

        assert_eq!(
            error.to_string(),
            "no cataloged Node runtime satisfies engines.node `>=27.0.0`; available: 22.23.1, 24.18.0, 24.19.0, 26.5.1, 26.7.0"
        );

        let graph = WorkspaceGraph::build([package(
            "",
            r#"{
                "name":"@acme/root",
                "engines":{"node":"26.5.1"},
                "packageManager":"pnpm@12.0.0"
            }"#,
        )])
        .unwrap();
        let error = render_typescript_build_file(
            &graph,
            CellRelativePathBuf::unchecked_new(String::new()),
            &listing,
        )
        .unwrap_err();

        assert_eq!(
            error.to_string(),
            "unsupported packageManager `pnpm@12.0.0`; BSMR supports exact pnpm 10.30.3 and 11.20.0 pins"
        );
    }
}
