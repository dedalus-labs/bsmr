//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Maps configured Cargo units to native rules without resolving features again.

use std::collections::BTreeMap;
use std::path::Component;
use std::path::Path;

use serde_json::to_string as json;

use super::RustGraphError;
use super::units::DebugInfo;
use super::units::Graph;
use super::units::Mode;
use super::units::Profile;
use super::units::SourceKind;
use super::units::Strip;
use super::units::StripSetting;
use super::units::Unit;
use super::unsupported;

/// Reject an unsupported execution contract before returning any build rules.
pub fn render(
    bytes: &[u8],
    root: &Path,
    cell: &str,
    toolchain: &str,
) -> Result<String, RustGraphError> {
    let graph = Graph::parse(bytes)?;
    if graph.workspace_root != root {
        return Err(unsupported("planner", "workspace root mismatch"));
    }
    if graph.roots.len() != 1 {
        return Err(unsupported("planner", "multiple entrypoint roots"));
    }
    let renderer = Renderer {
        graph: &graph,
        cell,
        toolchain,
    };
    let mut rules = String::new();
    for (index, unit) in graph.units.iter().enumerate() {
        rules.push_str(&renderer.unit(unit, index)?);
    }
    rules.push_str(&format!(
        "alias(name = \"root\", actual = \":unit_{}\", visibility = [\"PUBLIC\"])\n",
        graph.roots[0]
    ));
    Ok(rules)
}

/// The graph owns dependency indices. Its cell owns all admitted source trees.
struct Renderer<'a> {
    /// Configured units from one selected Cargo entrypoint.
    graph: &'a Graph,
    /// BSMR cell containing the captured workspace.
    cell: &'a str,
    /// Pinned compiler distribution used by every emitted rule.
    toolchain: &'a str,
}

impl Renderer<'_> {
    /// Keep each unit's compiler flags, environment and dependency aliases together.
    fn unit(&self, unit: &Unit, index: usize) -> Result<String, RustGraphError> {
        let rule = unit.rule()?;
        let (sources, source) = self.sources(unit)?;
        let dependencies = self.dependencies(unit)?;
        let primary = self
            .graph
            .roots
            .iter()
            .any(|root| self.graph.units[*root].package_id == unit.package_id);
        let environment = unit.environment(primary);
        let mut flags = profile_flags(&unit.profile, &unit.package_name)?;
        flags.extend(unit.package_lint_flags.iter().cloned());
        flags.extend(unit.rustflags.iter().cloned());
        let declared = unit
            .declared_features
            .iter()
            .map(json)
            .collect::<Result<Vec<_>, _>>()?
            .join(",");
        flags.extend([
            "--check-cfg".into(),
            format!("cfg(feature, values({declared}))"),
        ]);
        Ok(format!(
            "{rule}(name = \"unit_{index}\", crate = {}, crate_root = {}, edition = {}, mapped_srcs = {{{}: \"crate\"}}, named_deps = {}, features = {}, literal_rustc_flags = {}, literal_env = {}, verify_inputs = True, _rust_toolchain = {}, visibility = [\"PUBLIC\"])\n",
            json(&unit.target.name.replace('-', "_"))?,
            json(&source)?,
            json(&unit.target.edition)?,
            json(&sources)?,
            json(&dependencies)?,
            json(&unit.features)?,
            json(&flags)?,
            json(&environment)?,
            json(self.toolchain)?,
        ))
    }

    /// Locate compiler sources inside the tracked package that owns them.
    fn sources(&self, unit: &Unit) -> Result<(String, String), RustGraphError> {
        if !matches!(unit.source.kind, SourceKind::Path) {
            return Err(unsupported(
                &unit.package_name,
                "external source artifact materialization",
            ));
        }
        let package = unit
            .source
            .root
            .strip_prefix(&self.graph.workspace_root)
            .map_err(|_| RustGraphError::Outside(unit.source.root.clone()))?;
        let source = unit
            .target
            .src_path
            .strip_prefix(&unit.source.root)
            .map_err(|_| RustGraphError::Outside(unit.target.src_path.clone()))?;
        if source
            .components()
            .any(|part| matches!(part, Component::ParentDir))
        {
            return Err(unsupported(
                &unit.package_name,
                "source outside its declared package tree",
            ));
        }
        Ok((
            format!(
                "{}//{}:__bsmr_sources",
                self.cell,
                package.to_string_lossy().replace('\\', "/")
            ),
            format!("crate/{}", source.to_string_lossy().replace('\\', "/")),
        ))
    }

    /// Preserve Cargo's extern names while rejecting unsupported native dependency kinds.
    fn dependencies<'a>(
        &self,
        unit: &'a Unit,
    ) -> Result<BTreeMap<&'a str, String>, RustGraphError> {
        let mut dependencies = BTreeMap::new();
        for dependency in &unit.dependencies {
            if dependency.public || dependency.noprelude || dependency.nounused {
                return Err(unsupported(
                    &unit.package_name,
                    "dependency compiler modifiers",
                ));
            }
            let target = &self.graph.units[dependency.index];
            if target.target.kind != ["lib"] && target.target.kind != ["rlib"] {
                return Err(unsupported(&unit.package_name, "non-library dependency"));
            }
            if dependencies
                .insert(
                    dependency.extern_crate_name.as_str(),
                    format!(":unit_{}", dependency.index),
                )
                .is_some()
            {
                return Err(unsupported(
                    &unit.package_name,
                    "duplicate dependency alias",
                ));
            }
        }
        Ok(dependencies)
    }
}

impl Unit {
    /// Admit only compiler modes whose execution requirements are represented natively.
    fn rule(&self) -> Result<&'static str, RustGraphError> {
        let library = self.target.kind == ["lib"] || self.target.kind == ["rlib"];
        if !library && self.target.kind != ["bin"] {
            return Err(unsupported(
                &self.package_name,
                &format!(
                    "target kind {:?} requires qualified native execution",
                    self.target.kind
                ),
            ));
        }
        if self.platform.is_some() || self.linker.is_some() {
            return Err(unsupported(
                &self.package_name,
                "configured target or linker requires qualified native execution",
            ));
        }
        if self.package_links.is_some() || self.target.crate_types != self.target.kind {
            return Err(unsupported(
                &self.package_name,
                "native links or additional crate types",
            ));
        }
        match self.mode {
            Mode::Build if library => Ok("rust_library"),
            Mode::Build => Ok("rust_binary"),
            Mode::Test => Ok("rust_test"),
            Mode::Check | Mode::RunCustomBuild => Err(unsupported(
                &self.package_name,
                "compiler mode requires qualified native execution",
            )),
        }
    }

    /// Supply Cargo variables whose values do not depend on the checkout's absolute path.
    fn environment(&self, primary: bool) -> BTreeMap<String, String> {
        let mut environment = self.package_environment.clone();
        environment.insert(
            "CARGO_CRATE_NAME".into(),
            self.target.name.replace('-', "_"),
        );
        if self.target.kind == ["bin"] {
            environment.insert("CARGO_BIN_NAME".into(), self.target.name.clone());
        }
        if primary {
            environment.insert("CARGO_PRIMARY_PACKAGE".into(), "1".into());
        }
        environment
    }
}

/// Translate effective profiles whose linker requirements need no graph-wide propagation.
fn profile_flags(profile: &Profile, package: &str) -> Result<Vec<String>, RustGraphError> {
    if profile.codegen_backend.is_some() || !["off", "false"].contains(&profile.lto.as_str()) {
        return Err(unsupported(package, "codegen backend or cross-crate LTO"));
    }
    let mut flags = vec![
        format!("-Copt-level={}", profile.opt_level),
        format!("-Cdebug-assertions={}", profile.debug_assertions),
        format!("-Coverflow-checks={}", profile.overflow_checks),
        format!("-Cpanic={}", profile.panic),
        format!("-Crpath={}", profile.rpath),
        "-Cembed-bitcode=no".into(),
    ];
    if profile.lto == "off" {
        flags.push("-Clto=off".into());
    }
    if let Some(units) = profile.codegen_units {
        flags.push(format!("-Ccodegen-units={units}"));
    }
    flags.push(match &profile.debuginfo {
        DebugInfo::Level(level) => format!("-Cdebuginfo={level}"),
        DebugInfo::Named(name) => format!("-Cdebuginfo={name}"),
    });
    if let Some(split) = &profile.split_debuginfo {
        flags.push(format!("-Csplit-debuginfo={split}"));
    }
    match &profile.strip {
        StripSetting::Deferred(Strip::None) | StripSetting::Resolved(Strip::None) => {}
        StripSetting::Deferred(Strip::Named(name)) | StripSetting::Resolved(Strip::Named(name)) => {
            flags.push(format!("-Cstrip={name}"))
        }
    }
    Ok(flags)
}

#[cfg(test)]
mod tests {
    use super::*;
    const GRAPH: &[u8] = include_bytes!("../../../../tools/cargo/fixtures/unit.json");

    #[test]
    fn configured_inputs_preserve_literal_metadata() {
        let rules = render(GRAPH, Path::new("/workspace"), "root", "root//:rust").unwrap();
        assert!(rules.contains("literal_env ="));
        assert!(rules.contains("Literal $(location :absent)"));
        assert!(rules.contains("\"enabled\""));
    }

    #[test]
    fn configured_inputs_cannot_escape_the_captured_graph() {
        let mut graph: serde_json::Value = serde_json::from_slice(GRAPH).unwrap();
        assert!(render(GRAPH, Path::new("/other"), "root", "root//:rust").is_err());
        for path in [
            "/outside/lib.rs",
            "/workspace-neighbor/lib.rs",
            "/workspace/../outside.rs",
        ] {
            graph["units"][0]["target"]["src_path"] = path.into();
            assert!(
                render(
                    &serde_json::to_vec(&graph).unwrap(),
                    Path::new("/workspace"),
                    "root",
                    "root//:rust"
                )
                .is_err()
            );
        }
        graph["roots"] = serde_json::json!([999]);
        assert!(Graph::parse(&serde_json::to_vec(&graph).unwrap()).is_err());
    }
}
