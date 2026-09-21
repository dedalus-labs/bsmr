//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Lowers Cargo's resolved, hook-free local graph into the existing Rust prelude.

use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;
use serde_json::to_string as json;

use super::RustGraphError;
use super::unsupported;

#[derive(Deserialize)]
struct Metadata {
    /// Cargo metadata schema, currently required to be version one.
    version: u32,
    /// Absolute root used to reject inputs outside the captured workspace.
    workspace_root: PathBuf,
    /// All resolved packages, including dependencies.
    packages: Vec<Package>,
    /// Cargo resolution is required before any rules can be emitted.
    resolve: Option<Resolve>,
}

#[derive(Deserialize)]
struct Package {
    /// Opaque Cargo identity used to join packages and resolved nodes.
    id: String,
    /// Cargo name, preserved for dependency aliases and crate names.
    name: String,
    /// Package version exposed to compilation through CARGO_PKG_VERSION.
    version: String,
    /// A nonempty registry or Git source is outside this local import contract.
    source: Option<String>,
    /// Absolute manifest path that determines the package boundary.
    manifest_path: PathBuf,
    /// Cargo-discovered targets, each checked before lowering.
    targets: Vec<Target>,
    /// Native link ownership is unsupported by this importer.
    links: Option<String>,
}

#[derive(Deserialize)]
struct Target {
    /// Cargo name, preserved for dependency aliases and crate names.
    name: String,
    /// Only ordinary library and binary targets are admitted.
    kind: Vec<String>,
    /// Must match the admitted target kind.
    crate_types: Vec<String>,
    /// Entry point that must remain inside its package.
    src_path: PathBuf,
    /// Rust language edition supplied to the native rule.
    edition: String,
    /// Absent means Cargo enables the unit test harness.
    test: Option<bool>,
    #[serde(default, rename = "required-features")]
    /// Nonempty feature gates are unsupported.
    required_features: Vec<String>,
}

#[derive(Deserialize)]
struct Resolve {
    /// Dependency edges resolved by Cargo, keyed by opaque identity.
    nodes: Vec<Node>,
}

#[derive(Deserialize)]
struct Node {
    /// Opaque Cargo identity used to join packages and resolved nodes.
    id: String,
    /// Resolved extern names and their owning packages.
    deps: Vec<Dependency>,
    /// Activated features, rejected until configured units are modeled.
    features: Vec<String>,
}

#[derive(Deserialize)]
struct Dependency {
    /// Cargo name, preserved for dependency aliases and crate names.
    name: String,
    /// Opaque identity of the dependency package.
    pkg: String,
    /// Each use must be an unconditional ordinary dependency.
    dep_kinds: Vec<DependencyKind>,
}

#[derive(Deserialize)]
struct DependencyKind {
    /// Absent means an ordinary dependency, rather than build or dev.
    kind: Option<String>,
    /// Nonempty target predicates are unsupported.
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
    let directory = package.directory();
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
        rendered.push_str(&target.render(package, &dependencies, toolchain, false)?);
        if target.test.unwrap_or(true) {
            rendered.push_str(&target.render(package, &dependencies, toolchain, true)?);
        }
    }
    rendered.push_str(&package.alias(relative)?);
    Ok((root.join(relative).join("BUILD.bsmr"), rendered))
}

impl Package {
    /// Cargo always reports an absolute manifest file inside its package directory.
    fn directory(&self) -> &Path {
        self.manifest_path
            .parent()
            .expect("Cargo manifest has a directory")
    }

    /// Add a package-path alias when a package has one ordinary target.
    fn alias(&self, relative: &Path) -> Result<String, RustGraphError> {
        let [target] = self.targets.as_slice() else {
            return Ok(String::new());
        };
        let alias = relative
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or(&self.name);
        if alias.starts_with("__bsmr_") {
            return Err(unsupported(
                &self.name,
                "target names beginning with `__bsmr_` are reserved",
            ));
        }
        let actual = if target.kind == ["lib"] {
            "lib"
        } else {
            &target.name
        };
        if alias == actual {
            return Ok(String::new());
        }
        Ok(format!(
            "alias(name = {}, actual = {}, tests = {}, visibility = [\"PUBLIC\"])\n",
            json(alias)?,
            json(&format!(":{actual}"))?,
            json(&target.test_labels())?,
        ))
    }
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
            .directory()
            .strip_prefix(root)
            .map_err(|_| RustGraphError::Outside(target.manifest_path.clone()))?;
        dependencies.insert(
            dependency.name.clone(),
            format!("root//{}:lib", path.to_string_lossy().replace('\\', "/")),
        );
    }
    Ok(dependencies)
}

impl Target {
    /// Reuse native Rust rules with explicit crate inputs and resolved extern names.
    fn render(
        &self,
        package: &Package,
        dependencies: &BTreeMap<String, String>,
        toolchain: &str,
        test: bool,
    ) -> Result<String, RustGraphError> {
        let is_library = self.validate(&package.name)?;
        let source = self
            .src_path
            .strip_prefix(package.directory())
            .map_err(|_| RustGraphError::Outside(self.src_path.clone()))?;
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
            format!("__bsmr_test_{}_{}", self.kind[0], self.name)
        } else if is_library {
            "lib".to_owned()
        } else {
            self.name.clone()
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
            json(&name)?,
            json(&self.name.replace('-', "_"))?,
            json(&source.to_string_lossy().replace('\\', "/"))?,
            json(&self.edition)?,
            json(&named_deps)?,
            json(&if test { Vec::new() } else { self.test_labels() })?,
            json(&environment)?,
            json(toolchain)?,
        ))
    }

    /// Reject unsupported semantics and names before rendering a rule.
    fn validate(&self, package: &str) -> Result<bool, RustGraphError> {
        if self.kind != ["lib"] && self.kind != ["bin"] {
            return Err(unsupported(
                package,
                &format!("target kind {:?}", self.kind),
            ));
        }
        if self.crate_types != self.kind || !self.required_features.is_empty() {
            return Err(unsupported(package, "crate types or feature-gated targets"));
        }
        let is_library = self.kind == ["lib"];
        if !is_library && self.name.starts_with("__bsmr_") {
            return Err(unsupported(
                package,
                "target names beginning with `__bsmr_` are reserved",
            ));
        }
        if !is_library && self.name == "lib" {
            return Err(unsupported(package, "binary target name `lib` is reserved"));
        }
        Ok(is_library)
    }
    /// Associate only Cargo-enabled unit tests with a build target.
    fn test_labels(&self) -> Vec<String> {
        if self.test.unwrap_or(true) {
            vec![format!(":__bsmr_test_{}_{}", self.kind[0], self.name)]
        } else {
            Vec::new()
        }
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

    /// Lower a captured metadata fixture with the requested toolchain label.
    fn fixture(
        value: &Value,
        toolchain: &str,
    ) -> Result<BTreeMap<PathBuf, String>, RustGraphError> {
        render(
            &serde_json::to_vec(value).unwrap(),
            Path::new("/repo"),
            toolchain,
        )
    }

    #[test]
    fn package_path_selects_a_differently_named_binary() {
        let mut metadata = metadata();
        metadata["packages"][1]["targets"][0]["name"] = "probe_app".into();
        let files = fixture(&metadata, "root//:rust").unwrap();
        assert!(files[Path::new("/repo/app/BUILD.bsmr")].contains(
            "alias(name = \"app\", actual = \":probe_app\", tests = [\":__bsmr_test_bin_probe_app\"]"
        ));
    }

    #[test]
    fn associates_only_enabled_unit_tests() {
        let mut metadata = metadata();
        metadata["packages"][1]["targets"][0]["test"] = false.into();
        let files = fixture(&metadata, "root//:rust").unwrap();
        let core = &files[Path::new("/repo/core/BUILD.bsmr")];
        assert!(core.contains("rust_test("));
        assert!(core.contains("tests = [\":__bsmr_test_lib_core\"]"));
        let app = &files[Path::new("/repo/app/BUILD.bsmr")];
        assert!(!app.contains("rust_test("));
        assert!(app.contains("tests = []"));
    }

    #[test]
    fn imports_resolved_edges_into_native_rules() {
        let files = fixture(&metadata(), "toolchains//:rust").unwrap();
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
        let error = fixture(&value, "toolchains//:rust").unwrap_err();
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
            assert!(fixture(&value, "toolchains//:rust").is_err());
        }
    }

    #[test]
    fn rejects_feature_unification_without_a_configured_unit_graph() {
        let mut value = metadata();
        value["resolve"]["nodes"][0]["features"] = json!(["fast"]);
        let error = fixture(&value, "toolchains//:rust").unwrap_err();
        assert!(error.to_string().contains("feature variants"));
    }

    #[test]
    fn rejects_external_sources_and_escaping_crate_roots() {
        let mut value = metadata();
        value["packages"][0]["source"] = json!("registry+https://example.com/index");
        assert!(fixture(&value, "toolchains//:rust").is_err());
        value["packages"][0]["source"] = Value::Null;
        value["packages"][0]["targets"][0]["src_path"] = json!("/outside/lib.rs");
        assert!(fixture(&value, "toolchains//:rust").is_err());
    }
}
