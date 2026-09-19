//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Lowers Cargo's resolved, hook-free local graph into the existing Rust prelude.

use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;

/// Cargo graph input or an unsupported semantic boundary.
#[derive(Debug, bsmr_error::Error)]
#[bsmr(tag = Input)]
pub enum RustGraphError {
    #[error("invalid cargo metadata: {0}")]
    Metadata(#[source] serde_json::Error),
    #[error("native Rust import does not support {case} in `{package}`")]
    Unsupported { package: String, case: String },
    #[error("Cargo path `{0:?}` is outside the workspace")]
    Outside(PathBuf),
    #[error("Cargo graph is missing resolved package `{0}`")]
    Missing(String),
}

impl From<serde_json::Error> for RustGraphError {
    /// Preserve JSON decoding or serialization failure details.
    fn from(source: serde_json::Error) -> Self {
        Self::Metadata(source)
    }
}

#[derive(Deserialize)]
struct Metadata {
    version: u32,
    workspace_root: PathBuf,
    packages: Vec<Package>,
    resolve: Option<Resolve>,
}

#[derive(Deserialize)]
struct Package {
    id: String,
    name: String,
    version: String,
    source: Option<String>,
    manifest_path: PathBuf,
    targets: Vec<Target>,
    links: Option<String>,
}

#[derive(Deserialize)]
struct Target {
    name: String,
    kind: Vec<String>,
    crate_types: Vec<String>,
    src_path: PathBuf,
    edition: String,
    test: Option<bool>,
    #[serde(default, rename = "required-features")]
    required_features: Vec<String>,
}

#[derive(Deserialize)]
struct Resolve {
    nodes: Vec<Node>,
}

#[derive(Deserialize)]
struct Node {
    id: String,
    deps: Vec<Dependency>,
    features: Vec<String>,
}

#[derive(Deserialize)]
struct Dependency {
    name: String,
    pkg: String,
    dep_kinds: Vec<DependencyKind>,
}

#[derive(Deserialize)]
struct DependencyKind {
    kind: Option<String>,
    target: Option<String>,
}

/// Rejects unmodeled Cargo semantics before generating any build rules.
pub fn render(
    bytes: &[u8],
    root: &Path,
    toolchain: &str,
) -> Result<BTreeMap<PathBuf, String>, RustGraphError> {
    let metadata: Metadata = serde_json::from_slice(bytes)?;
    if metadata.version != 1 || metadata.workspace_root != root {
        return Err(unsupported(
            "workspace",
            "metadata version or workspace root mismatch",
        ));
    }
    let resolve = metadata
        .resolve
        .ok_or_else(|| unsupported("workspace", "unresolved metadata"))?;
    let packages: BTreeMap<_, _> = metadata
        .packages
        .iter()
        .map(|p| (p.id.as_str(), p))
        .collect();
    let nodes: BTreeMap<_, _> = resolve.nodes.iter().map(|n| (n.id.as_str(), n)).collect();
    let mut output = BTreeMap::new();
    for package in &metadata.packages {
        let (path, manifest) = render_package(package, root, &nodes, &packages, toolchain)?;
        output.insert(path, manifest);
    }
    Ok(output)
}

/// Render one package after validating its dependency context.
fn render_package(
    package: &Package,
    root: &Path,
    nodes: &BTreeMap<&str, &Node>,
    packages: &BTreeMap<&str, &Package>,
    toolchain: &str,
) -> Result<(PathBuf, String), RustGraphError> {
    if package.source.is_some() || package.links.is_some() {
        return Err(unsupported(
            &package.name,
            "external sources or native links",
        ));
    }
    let directory = package
        .manifest_path
        .parent()
        .expect("Cargo manifest has a directory");
    let relative = directory
        .strip_prefix(root)
        .map_err(|_| RustGraphError::Outside(directory.to_owned()))?;
    let node = nodes
        .get(package.id.as_str())
        .ok_or_else(|| RustGraphError::Missing(package.id.clone()))?;
    if !node.features.is_empty() {
        return Err(unsupported(
            &package.name,
            "feature variants require configured Cargo unit graphs",
        ));
    }
    let dependencies = resolved_dependencies(package, node, packages, root)?;
    let mut rendered = String::from(
        "# ===----------------------------------------------------------------------===\n\
         # Copyright (c) 2026 Dedalus Labs, Inc. and its contributors\n\
         # SPDX-License-Identifier: Apache-2.0\n\
         # ===----------------------------------------------------------------------===\n\n\
         # Private Cargo graph. Do not write this representation into the repository.\n\n",
    );
    for target in &package.targets {
        rendered.push_str(&render_target(
            package,
            target,
            directory,
            &dependencies,
            toolchain,
            false,
        )?);
        if target.test.unwrap_or(true) {
            rendered.push_str(&render_target(
                package,
                target,
                directory,
                &dependencies,
                toolchain,
                true,
            )?);
        }
    }
    if let [target] = package.targets.as_slice() {
        let alias = relative
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(&package.name);
        let actual = if target.kind == ["lib"] {
            "lib"
        } else {
            &target.name
        };
        if alias != actual {
            rendered.push_str(&format!(
                r#"alias(name = {}, actual = {}, tests = {}, visibility = ["PUBLIC"])
"#,
                serde_json::to_string(alias)?,
                serde_json::to_string(&format!(":{actual}"))?,
                serde_json::to_string(&test_labels(target))?,
            ));
        }
    }
    Ok((root.join(relative).join("BUILD.bsmr"), rendered))
}

/// Preserve Cargo's dependency aliases without reconstructing its resolver.
fn resolved_dependencies(
    package: &Package,
    node: &Node,
    packages: &BTreeMap<&str, &Package>,
    root: &Path,
) -> Result<BTreeMap<String, String>, RustGraphError> {
    let mut dependencies = BTreeMap::new();
    for dependency in &node.deps {
        for kind in &dependency.dep_kinds {
            if kind.target.is_some() || kind.kind.is_some() {
                return Err(unsupported(
                    &package.name,
                    "conditional, build, or dev dependencies",
                ));
            }
        }
        let target = packages
            .get(dependency.pkg.as_str())
            .ok_or_else(|| RustGraphError::Missing(dependency.pkg.clone()))?;
        if !target
            .targets
            .iter()
            .any(|t| t.kind == ["lib"] && t.crate_types == ["lib"])
        {
            return Err(unsupported(
                &target.name,
                "dependencies without an ordinary library",
            ));
        }
        let path = target
            .manifest_path
            .parent()
            .unwrap()
            .strip_prefix(root)
            .map_err(|_| RustGraphError::Outside(target.manifest_path.clone()))?;
        dependencies.insert(
            dependency.name.clone(),
            format!("root//{}:lib", path.to_string_lossy().replace('\\', "/")),
        );
    }
    Ok(dependencies)
}

/// Reuse native Rust rules with explicit crate inputs and resolved extern names.
fn render_target(
    package: &Package,
    target: &Target,
    directory: &Path,
    dependencies: &BTreeMap<String, String>,
    toolchain: &str,
    test: bool,
) -> Result<String, RustGraphError> {
    if target.kind != ["lib"] && target.kind != ["bin"] {
        return Err(unsupported(
            &package.name,
            &format!("target kind {:?}", target.kind),
        ));
    }
    if target.crate_types != target.kind || !target.required_features.is_empty() {
        return Err(unsupported(
            &package.name,
            "crate types or feature-gated targets",
        ));
    }
    let source = target
        .src_path
        .strip_prefix(directory)
        .map_err(|_| RustGraphError::Outside(target.src_path.clone()))?;
    let is_library = target.kind == ["lib"];
    let mut named_deps = dependencies.clone();
    if !is_library {
        for library in package.targets.iter().filter(|t| t.kind == ["lib"]) {
            named_deps.insert(library.name.replace('-', "_"), ":lib".to_owned());
        }
    }
    let environment = BTreeMap::from([
        ("CARGO_PKG_NAME", package.name.as_str()),
        ("CARGO_PKG_VERSION", package.version.as_str()),
    ]);
    let name = if test {
        format!("__bsmr_test_{}", target.name)
    } else if is_library {
        "lib".to_owned()
    } else {
        target.name.clone()
    };
    Ok(format!(
        "rust_{}(\n    name = {},\n    crate = {},\n    crate_root = {},\n    edition = {},\n    srcs = glob([\"**\"], exclude = [\"BUILD.bsmr\", \"target/**\", \"bsmr-out/**\", \".git/**\"]),\n    named_deps = {},\n    verify_inputs = True,\n    tests = {},\n    env = {},\n    _rust_toolchain = {},\n    visibility = [\"PUBLIC\"],\n)\n\n",
        if test {
            "test"
        } else if is_library {
            "library"
        } else {
            "binary"
        },
        serde_json::to_string(&name)?,
        serde_json::to_string(&target.name.replace('-', "_"))?,
        serde_json::to_string(&source.to_string_lossy().replace('\\', "/"))?,
        serde_json::to_string(&target.edition)?,
        serde_json::to_string(&named_deps)?,
        serde_json::to_string(&if test {
            Vec::new()
        } else {
            test_labels(target)
        })?,
        serde_json::to_string(&environment)?,
        serde_json::to_string(toolchain)?,
    ))
}

/// Associate only Cargo-enabled unit tests with a build target.
fn test_labels(target: &Target) -> Vec<String> {
    if target.test.unwrap_or(true) {
        vec![format!(":__bsmr_test_{}", target.name)]
    } else {
        Vec::new()
    }
}

/// Names the exact unsupported package contract.
fn unsupported(package: &str, case: &str) -> RustGraphError {
    RustGraphError::Unsupported {
        package: package.to_owned(),
        case: case.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::Value;
    use serde_json::json;

    use super::*;

    /// Return a resolved local workspace without feature variants.
    fn metadata() -> Value {
        json!({"version":1,"workspace_root":"/repo","packages":[
            {"id":"opaque-core","name":"core","version":"1.0.0","source":null,"links":null,"manifest_path":"/repo/core/Cargo.toml","targets":[{"name":"core","kind":["lib"],"crate_types":["lib"],"src_path":"/repo/core/src/lib.rs","edition":"2024"}]},
            {"id":"opaque-app","name":"app","version":"1.0.0","source":null,"links":null,"manifest_path":"/repo/app/Cargo.toml","targets":[{"name":"app","kind":["bin"],"crate_types":["bin"],"src_path":"/repo/app/src/main.rs","edition":"2024"}]}
        ],"resolve":{"nodes":[
            {"id":"opaque-core","deps":[],"features":[]},
            {"id":"opaque-app","deps":[{"name":"renamed_core","pkg":"opaque-core","dep_kinds":[{"kind":null,"target":null}]}],"features":[]}
        ]}})
    }

    #[test]
    fn package_path_selects_a_differently_named_binary() {
        let mut metadata = metadata();
        metadata["packages"][1]["targets"][0]["name"] = "probe_app".into();
        let files = render(
            &serde_json::to_vec(&metadata).unwrap(),
            Path::new("/repo"),
            "root//:rust",
        )
        .unwrap();
        assert!(files[Path::new("/repo/app/BUILD.bsmr")].contains(
            "alias(name = \"app\", actual = \":probe_app\", tests = [\":__bsmr_test_probe_app\"]"
        ));
    }

    #[test]
    fn associates_only_enabled_unit_tests() {
        let mut metadata = metadata();
        metadata["packages"][1]["targets"][0]["test"] = false.into();
        let files = render(
            &serde_json::to_vec(&metadata).unwrap(),
            Path::new("/repo"),
            "root//:rust",
        )
        .unwrap();
        let core = &files[Path::new("/repo/core/BUILD.bsmr")];
        assert!(core.contains("rust_test("));
        assert!(core.contains("tests = [\":__bsmr_test_core\"]"));
        let app = &files[Path::new("/repo/app/BUILD.bsmr")];
        assert!(!app.contains("rust_test("));
        assert!(app.contains("tests = []"));
    }

    #[test]
    fn imports_resolved_edges_into_native_rules() {
        let files = render(
            &serde_json::to_vec(&metadata()).unwrap(),
            Path::new("/repo"),
            "toolchains//:rust",
        )
        .unwrap();
        let app = &files[Path::new("/repo/app/BUILD.bsmr")];
        assert!(app.contains("rust_binary("));
        assert!(app.contains("\"renamed_core\":\"root//core:lib\""));
        assert!(!app.contains("cargo_build"));
        let core = &files[Path::new("/repo/core/BUILD.bsmr")];
        assert!(core.contains("rust_library("));
        assert!(!core.contains("cargo_build"));
    }

    #[test]
    fn rejects_build_scripts_before_emitting_graph() {
        let mut value = metadata();
        value["packages"][0]["targets"][0]["kind"] = json!(["custom-build"]);
        let error = render(
            &serde_json::to_vec(&value).unwrap(),
            Path::new("/repo"),
            "toolchains//:rust",
        )
        .unwrap_err();
        assert!(error.to_string().contains("custom-build"));
    }

    #[test]
    fn rejects_unmodeled_resolution_contexts() {
        for kind in [
            json!({"kind":"build","target":null}),
            json!({"kind":null,"target":"cfg(unix)"}),
        ] {
            let mut value = metadata();
            value["resolve"]["nodes"][1]["deps"][0]["dep_kinds"] = json!([kind]);
            assert!(
                render(
                    &serde_json::to_vec(&value).unwrap(),
                    Path::new("/repo"),
                    "toolchains//:rust"
                )
                .is_err()
            );
        }
    }

    #[test]
    fn rejects_feature_unification_without_a_configured_unit_graph() {
        let mut value = metadata();
        value["resolve"]["nodes"][0]["features"] = json!(["fast"]);
        let error = render(
            &serde_json::to_vec(&value).unwrap(),
            Path::new("/repo"),
            "toolchains//:rust",
        )
        .unwrap_err();
        assert!(error.to_string().contains("feature variants"));
    }

    #[test]
    fn rejects_external_sources_and_escaping_crate_roots() {
        let mut value = metadata();
        value["packages"][0]["source"] = json!("registry+https://example.com/index");
        assert!(
            render(
                &serde_json::to_vec(&value).unwrap(),
                Path::new("/repo"),
                "toolchains//:rust"
            )
            .is_err()
        );
        value["packages"][0]["source"] = Value::Null;
        value["packages"][0]["targets"][0]["src_path"] = json!("/outside/lib.rs");
        assert!(
            render(
                &serde_json::to_vec(&value).unwrap(),
                Path::new("/repo"),
                "toolchains//:rust"
            )
            .is_err()
        );
    }
}
