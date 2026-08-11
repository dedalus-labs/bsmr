//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Renders internal TypeScript rules from an authoritative pnpm workspace graph.

use std::fmt::Write;

use bsmr_core::cells::paths::CellRelativePathBuf;
use bsmr_core::package::package_relative_path::PackageRelativePath;

use super::WorkspaceGraph;
use crate::package_listing::listing::PackageListing;

const SOURCES_TARGET: &str = "__bsmr_sources";
const INSTALL_TARGET: &str = "__bsmr_dependencies";

/// A native TypeScript package cannot be lowered without satisfying these invariants.
#[derive(Debug, bsmr_error::Error)]
#[bsmr(tag = Input)]
pub enum NativeTypeScriptBuildError {
    /// The requested package must be selected by the authoritative workspace manifest.
    #[error("package `{0}` is not selected by pnpm-workspace.yaml")]
    UnknownPackage(CellRelativePathBuf),
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
    /// Formatting internal Starlark into a string should be infallible.
    #[error("failed to render native TypeScript build graph")]
    Render(std::fmt::Error),
}

/// Renders the private Starlark bridge consumed by BSMR's build interpreter.
pub fn render_typescript_build_file(
    graph: &WorkspaceGraph,
    package_root: CellRelativePathBuf,
    listing: &PackageListing,
) -> Result<String, NativeTypeScriptBuildError> {
    let Some(package_name) = graph.package_name_at_root(&package_root) else {
        return Err(NativeTypeScriptBuildError::UnknownPackage(package_root));
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
    let has_typescript = package_files.iter().any(|file| is_typescript(file));
    let has_tsconfig = package_files.iter().any(|file| *file == "tsconfig.json");
    let has_tsdown = package_files.iter().any(|file| *file == "tsdown.config.ts");
    if has_typescript && (!has_tsconfig || !has_tsdown) {
        return Err(NativeTypeScriptBuildError::MissingCompilerConfig(
            package_root,
        ));
    }

    let mut output = String::new();
    if package_root.is_empty() {
        render_install(listing, &mut output)?;
    }
    if !has_typescript {
        return Ok(output);
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
    Ok(output)
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
                project.root != *package_root
                    && !project.root.is_empty()
                    && file
                        .strip_prefix(project.root.as_str())
                        .is_some_and(|suffix| suffix.starts_with('/'))
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

/// Recognizes TypeScript source modules without guessing JavaScript packages.
fn is_typescript(path: &str) -> bool {
    [".ts", ".tsx", ".mts", ".cts"]
        .into_iter()
        .any(|extension| path.ends_with(extension))
        && path != "tsdown.config.ts"
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
        .unwrap();

        assert!(build.contains("name = \"api\""));
        assert!(build.contains("\"root//packages/core:__bsmr_sources\""));
        assert!(build.contains("\"apps/api/src/index.ts\": \"src/index.ts\""));
        assert!(!build.contains(":lib"));
    }

    #[test]
    fn invariant_workspace_install_uses_manifests_not_source_tree() {
        let graph = WorkspaceGraph::build([
            package("", r#"{"name":"@acme/root"}"#),
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
        .unwrap();

        assert!(build.contains("name = \"__bsmr_dependencies\""));
        assert!(build.contains("\"packages/api/package.json\""));
        assert!(!build.contains("packages/api/src/index.ts"));
    }
}
