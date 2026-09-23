//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Maps configured Cargo units to native rules without resolving features again.

#[path = "scripts.rs"]
mod scripts;

use std::collections::BTreeMap;
use std::path::Component;
use std::path::Path;

use serde_json::to_string as json;

use super::RustGraphError;
use super::sources::Sources;
use super::units::DebugInfo;
use super::units::Graph;
use super::units::Lto;
use super::units::Mode;
use super::units::Strip;
use super::units::StripSetting;
use super::units::Unit;
use super::unsupported;

/// Whether the selected executor isolates code that runs inside the compiler.
#[derive(Clone, Copy)]
pub(super) enum CodeExecution {
    /// Compilation may not execute code supplied by a package.
    CompilerOnly,
    /// Package code sees frozen inputs and its descendants end with the action.
    DeclaredInputs,
}

/// Reject an unsupported execution contract before returning any build rules.
pub(super) fn render(
    bytes: &[u8],
    root: &Path,
    cell: &str,
    toolchain: &str,
    execution: CodeExecution,
) -> Result<String, RustGraphError> {
    let graph = Graph::parse(bytes)?;
    if graph.workspace_root != root {
        return Err(unsupported("planner", "workspace root mismatch"));
    }
    if graph.roots.len() != 1 {
        return Err(unsupported("planner", "multiple entrypoint roots"));
    }
    let sources = Sources::render(&graph, cell)?;
    let renderer = Renderer {
        graph: &graph,
        sources: &sources.targets,
        toolchain,
        execution,
    };
    let mut rules = sources.rules;
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
    /// Declared source artifacts owned by each package.
    sources: &'a BTreeMap<&'a str, String>,
    /// Pinned compiler distribution used by every emitted rule.
    toolchain: &'a str,
    /// Verified execution policy captured before analysis reuse.
    execution: CodeExecution,
}

/// A compiler consumes extern crates and at most one output from its own build script.
#[derive(Default)]
struct Dependencies<'a> {
    /// Cargo's compiler-visible names for library dependencies.
    crates: BTreeMap<&'a str, String>,
    /// Graph index of this package's script execution, when present.
    script: Option<usize>,
}

impl Renderer<'_> {
    /// Keep each unit's compiler flags, environment and dependency aliases together.
    fn unit(&self, unit: &Unit, index: usize) -> Result<String, RustGraphError> {
        let rule = unit.rule(self.execution)?;
        if matches!(unit.mode, Mode::RunCustomBuild) {
            return self.script(unit, index);
        }
        let output = if unit.target.kind == ["proc-macro"] {
            ", proc_macro = True, default_output = \"library\""
        } else if rule == "rust_library" {
            ", default_output = \"library\""
        } else {
            ""
        };
        let (sources, source) = self.sources(unit)?;
        let dependencies = self.dependencies(unit)?;
        let sources = match dependencies.script {
            Some(script) => format!(":unit_{script}[cwd]"),
            None => sources,
        };
        let generated = dependencies.script.map(|script| format!(
            ", srcs = [\":unit_{script}[out_dir]\"], rustc_flags = [\"@$(location :unit_{script}[rustc_flags])\"], env = {{\"OUT_DIR\": \"$(location :unit_{script}[out_dir])\"}}"
        )).unwrap_or_default();
        let primary = self
            .graph
            .roots
            .iter()
            .any(|root| self.graph.units[*root].package_id == unit.package_id);
        let environment = unit.environment(primary);
        let mut flags = profile_flags(unit)?;
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
            "{rule}(name = \"unit_{index}\", crate = {}, crate_root = {}, edition = {}, mapped_srcs = {{{}: \"crate\"}}, named_deps = {}, features = {}, literal_rustc_flags = {}, literal_env = {}, verify_inputs = True, _rust_toolchain = {}, visibility = [\"PUBLIC\"]{output}{generated})\n",
            json(&unit.target.name.replace('-', "_"))?,
            json(&source)?,
            json(&unit.target.edition)?,
            json(&sources)?,
            json(&dependencies.crates)?,
            json(&unit.features)?,
            json(&flags)?,
            json(&environment)?,
            json(self.toolchain)?,
        ))
    }

    /// Locate compiler sources inside the tracked package that owns them.
    fn sources(&self, unit: &Unit) -> Result<(String, String), RustGraphError> {
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
            self.sources[unit.package_id.as_str()].clone(),
            format!("crate/{}", source.to_string_lossy().replace('\\', "/")),
        ))
    }

    /// Preserve Cargo's extern names while rejecting unsupported native dependency kinds.
    fn dependencies<'a>(&self, unit: &'a Unit) -> Result<Dependencies<'a>, RustGraphError> {
        let mut dependencies = Dependencies::default();
        for dependency in &unit.dependencies {
            if dependency.public || dependency.noprelude || dependency.nounused {
                return Err(unsupported(
                    &unit.package_name,
                    "dependency compiler modifiers",
                ));
            }
            let target = &self.graph.units[dependency.index];
            if matches!(target.mode, Mode::RunCustomBuild) {
                if target.package_id != unit.package_id
                    || dependencies.script.replace(dependency.index).is_some()
                {
                    return Err(unsupported(
                        &unit.package_name,
                        "build-script output ownership",
                    ));
                }
                continue;
            }
            if target.target.kind != ["lib"]
                && target.target.kind != ["rlib"]
                && target.target.kind != ["proc-macro"]
            {
                return Err(unsupported(
                    &unit.package_name,
                    &format!(
                        "dependency kind {:?} requires qualified native execution",
                        target.target.kind
                    ),
                ));
            }
            if dependencies
                .crates
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
    fn rule(&self, execution: CodeExecution) -> Result<&'static str, RustGraphError> {
        let macro_target = self.target.kind == ["proc-macro"];
        let script = self.target.kind == ["custom-build"];
        if (macro_target || script) && matches!(execution, CodeExecution::CompilerOnly) {
            return Err(unsupported(
                &self.package_name,
                "package code requires a verified declared-input executor",
            ));
        }
        let library = self.target.kind == ["lib"] || self.target.kind == ["rlib"] || macro_target;
        if !library && !script && self.target.kind != ["bin"] {
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
        let crate_types_match = if script {
            self.target.crate_types == ["bin"]
        } else {
            self.target.crate_types == self.target.kind
        };
        if !crate_types_match {
            return Err(unsupported(&self.package_name, "additional crate types"));
        }
        match self.mode {
            Mode::Build if library => Ok("rust_library"),
            Mode::Build => Ok("rust_binary"),
            Mode::Test => Ok("rust_test"),
            Mode::RunCustomBuild if script => Ok("buildscript_run"),
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

/// Keep bitcode in libraries for cross-crate optimization at the executable link.
fn profile_flags(unit: &Unit) -> Result<Vec<String>, RustGraphError> {
    let profile = &unit.profile;
    if profile.codegen_backend.is_some() {
        return Err(unsupported(&unit.package_name, "codegen backend"));
    }
    let mut flags = vec![
        format!("-Copt-level={}", profile.opt_level),
        format!("-Cdebug-assertions={}", profile.debug_assertions),
        format!("-Coverflow-checks={}", profile.overflow_checks),
        format!("-Cpanic={}", profile.panic),
        format!("-Crpath={}", profile.rpath),
    ];
    let links = matches!(unit.mode, Mode::Test) || unit.target.kind == ["bin"];
    let optimization: &[&str] = match (profile.lto, links) {
        (Lto::Off, _) => &["-Clto=off", "-Cembed-bitcode=no"],
        (Lto::Local, _) => &["-Cembed-bitcode=no"],
        // Libraries retain object code too, so their rlibs also work without LTO.
        (Lto::Fat | Lto::Thin, false) => &["-Cembed-bitcode=yes"],
        (Lto::Fat, true) => &["-Clto=fat", "-Cembed-bitcode=yes"],
        (Lto::Thin, true) => &["-Clto=thin", "-Cembed-bitcode=yes"],
    };
    flags.extend(optimization.iter().map(|flag| (*flag).to_owned()));
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
    fn invariant_unknown_lto_policy_is_rejected() {
        let mut graph: serde_json::Value = serde_json::from_slice(GRAPH).unwrap();
        graph["units"][0]["profile"]["lto"] = "unknown".into();
        assert!(Graph::parse(&serde_json::to_vec(&graph).unwrap()).is_err());
    }

    #[test]
    fn configured_inputs_preserve_literal_metadata() {
        let rules = render(
            GRAPH,
            Path::new("/workspace"),
            "root",
            "root//:rust",
            CodeExecution::CompilerOnly,
        )
        .unwrap();
        assert!(rules.contains("literal_env ="));
        assert!(rules.contains("Literal $(location :absent)"));
        assert!(rules.contains("\"enabled\""));
    }

    #[test]
    fn configured_inputs_cannot_escape_the_captured_graph() {
        let mut graph: serde_json::Value = serde_json::from_slice(GRAPH).unwrap();
        assert!(
            render(
                GRAPH,
                Path::new("/other"),
                "root",
                "root//:rust",
                CodeExecution::CompilerOnly
            )
            .is_err()
        );
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
                    "root//:rust",
                    CodeExecution::CompilerOnly,
                )
                .is_err()
            );
        }
        graph["roots"] = serde_json::json!([999]);
        assert!(Graph::parse(&serde_json::to_vec(&graph).unwrap()).is_err());
    }

    #[test]
    fn invariant_generated_code_requires_declared_input_execution() {
        let mut graph: serde_json::Value = serde_json::from_slice(GRAPH).unwrap();
        graph["units"][0]["target"]["kind"] = serde_json::json!(["proc-macro"]);
        graph["units"][0]["target"]["crate_types"] = serde_json::json!(["proc-macro"]);
        let bytes = serde_json::to_vec(&graph).unwrap();
        let render = |execution| {
            render(
                &bytes,
                Path::new("/workspace"),
                "root",
                "root//:rust",
                execution,
            )
        };
        assert!(
            render(CodeExecution::CompilerOnly)
                .unwrap_err()
                .to_string()
                .contains("verified declared-input executor")
        );
        assert!(
            render(CodeExecution::DeclaredInputs)
                .unwrap()
                .contains("proc_macro = True")
        );
    }
}
