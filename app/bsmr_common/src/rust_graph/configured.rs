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
use super::libraries;
use super::sources::Sources;
use super::units::DebugInfo;
use super::units::Graph;
use super::units::Lto;
use super::units::Mode;
use super::units::SourceArtifact;
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
    rules.push_str("load(\"@prelude//toolchains:cxx.bzl\", \"system_cxx_toolchain\")\n");
    for (index, unit) in graph.units.iter().enumerate() {
        if let Some(linker) = &unit.linker {
            rules.push_str(&format!(
                "system_cxx_toolchain(name = \"linker_{index}\", linker = {})\n",
                json(linker)?,
            ));
        }
        rules.push_str(&renderer.unit(unit, index)?);
    }
    rules.push_str(&format!(
        "load(\"@prelude//rust:cargo_outputs.bzl\", \"cargo_outputs\")\n\
         cargo_outputs(name = \"root\", actual = {}, outputs = {}, visibility = [\"PUBLIC\"])\n",
        json(&libraries::root(&graph))?,
        json(&libraries::outputs(&graph))?,
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
    /// Executables Cargo builds for integration tests, separate from extern crates.
    binaries: BTreeMap<String, String>,
}

/// One declared workspace view and the package projected from it.
struct TestSources {
    /// Native rule which assembles the selected workspace package inputs.
    rule: String,
    /// Complete view retained as a test resource.
    root: String,
    /// Package directory used for compilation and test execution.
    package: String,
}

impl Renderer<'_> {
    /// Keep each unit's compiler flags, environment and dependency aliases together.
    fn unit(&self, unit: &Unit, index: usize) -> Result<String, RustGraphError> {
        let rule = unit.rule(self.execution)?;
        if matches!(unit.mode, Mode::RunCustomBuild) {
            return self.script(unit, index);
        }
        let mut output = unit.output(rule, index)?;
        let (sources, source) = self.sources(unit)?;
        let dependencies = self.dependencies(unit)?;
        let mut artifact_env = dependencies.binaries;
        let mut sources = match dependencies.script {
            Some(script) => format!(":unit_{script}[cwd]"),
            None => sources,
        };
        let generated = dependencies.script.map(|script| format!(
            ", srcs = [\":unit_{script}[out_dir]\"], rustc_flags = [\"@$(location :unit_{script}[rustc_flags])\"]"
        )).unwrap_or_default();
        if let Some(script) = dependencies.script {
            artifact_env.insert(
                "OUT_DIR".into(),
                format!("$(location :unit_{script}[out_dir])"),
            );
        }
        let mut declarations = String::new();
        if rule == "rust_test" {
            let view = self.test_sources(unit, index, &sources)?;
            declarations = view.rule;
            sources = view.package;
            output.push_str(&format!(", resources = [{}]", json(&view.root)?));
            output.push_str(if unit.harness {
                ", framework = True"
            } else {
                ", framework = False"
            });
            artifact_env.insert(
                "CARGO_MANIFEST_DIR".into(),
                format!("$(location {sources})"),
            );
            output.push_str(&format!(", run_cwd = {}", json(&sources)?));
        }
        output.push_str(&format!(", env = {}", json(&artifact_env)?));
        let primary = self
            .graph
            .roots
            .iter()
            .any(|root| self.graph.units[*root].package_id == unit.package_id);
        let environment = unit.environment(primary);
        let flags = unit.flags()?;
        Ok(format!(
            "{declarations}{rule}(name = \"unit_{index}\", crate = {}, crate_root = {}, edition = {}, mapped_srcs = {{{}: \"crate\"}}, named_deps = {}, features = {}, literal_rustc_flags = {}, literal_env = {}, verify_inputs = True, _rust_toolchain = {}, visibility = [\"PUBLIC\"]{output}{generated})\n",
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
            if matches!(unit.mode, Mode::Test)
                && matches!(target.mode, Mode::Build)
                && target.target.kind == ["bin"]
                && target.package_id == unit.package_id
            {
                dependencies.binaries.insert(
                    format!("CARGO_BIN_EXE_{}", target.target.name),
                    format!("$(location :unit_{})", dependency.index),
                );
                continue;
            }
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
            if !(libraries::is_library(&target.target.kind)
                && libraries::has_rust_library(&target.target.kind))
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

    /// Retain local dependency fixtures while preserving the primary script's source output.
    fn test_sources(
        &self,
        unit: &Unit,
        index: usize,
        primary: &str,
    ) -> Result<TestSources, RustGraphError> {
        let mut packages = BTreeMap::new();
        for dependency in &self.graph.units {
            if matches!(&dependency.source.artifact, SourceArtifact::Workspace) {
                let path = dependency
                    .source
                    .root
                    .strip_prefix(&self.graph.workspace_root)
                    .map_err(|_| RustGraphError::Outside(dependency.source.root.clone()))?;
                packages.insert(
                    path.to_string_lossy().replace('\\', "/"),
                    self.sources[dependency.package_id.as_str()].as_str(),
                );
            }
        }
        let path = unit
            .source
            .root
            .strip_prefix(&self.graph.workspace_root)
            .map_err(|_| RustGraphError::Outside(unit.source.root.clone()))?;
        let path = path.to_string_lossy().replace('\\', "/");
        packages.insert(path.clone(), primary);
        let name = format!("test_sources_{index}");
        Ok(TestSources {
            rule: format!(
                "load(\"@prelude//rust:cargo_test_sources.bzl\", \"cargo_test_sources\")\n\
                 cargo_test_sources(name = {}, packages = {}, package = {})\n",
                json(&name)?,
                json(&packages)?,
                json(&path)?,
            ),
            root: format!(":{name}"),
            package: format!(":{name}[package]"),
        })
    }
}

impl Unit {
    /// Preserve profile, package lint and project flag order for one compiler action.
    fn flags(&self) -> Result<Vec<String>, RustGraphError> {
        let mut flags = profile_flags(self)?;
        if matches!(self.mode, Mode::Test) && !self.harness {
            flags.push("--cfg=test".into());
        }
        flags.extend(self.package_lint_flags.iter().cloned());
        flags.extend(self.rustflags.iter().cloned());
        let declared = self
            .declared_features
            .iter()
            .map(json)
            .collect::<Result<Vec<_>, _>>()?
            .join(",");
        flags.extend([
            "--check-cfg".into(),
            format!("cfg(feature, values({declared}))"),
        ]);
        Ok(flags)
    }

    /// Preserve native library providers and the Cargo library file name.
    fn output(&self, rule: &str, index: usize) -> Result<String, RustGraphError> {
        let mut output = if self.target.kind == ["proc-macro"] {
            ", proc_macro = True, default_output = \"library\""
        } else if rule == "rust_library" {
            ", default_output = \"library\""
        } else {
            ""
        }
        .to_owned();
        if self.linker.is_some() {
            output.push_str(&format!(", _cxx_toolchain = \":linker_{index}\""));
        }
        if rule == "rust_library" {
            output.push_str(&format!(
                ", soname = {}",
                json(&format!("lib{}.$(ext)", self.target.name.replace('-', "_")))?,
            ));
        }
        Ok(output)
    }

    /// Admit only compiler modes whose execution requirements are represented natively.
    fn rule(&self, execution: CodeExecution) -> Result<&'static str, RustGraphError> {
        let macro_target = self.target.kind == ["proc-macro"];
        let script = self.target.kind == ["custom-build"];
        let linker = self.linker.is_some()
            || self.rustflags.iter().any(|flag| {
                ["link-arg=", "-Clink-arg=", "--codegen=link-arg="]
                    .iter()
                    .any(|prefix| flag.starts_with(prefix))
            });
        if (macro_target || script || linker) && matches!(execution, CodeExecution::CompilerOnly) {
            return Err(unsupported(
                &self.package_name,
                "package code or configured linking requires a verified declared-input executor",
            ));
        }
        let library = libraries::is_library(&self.target.kind) || macro_target;
        let integration = self.target.kind == ["test"] && matches!(self.mode, Mode::Test);
        if !library && !script && !integration && self.target.kind != ["bin"] {
            return Err(unsupported(
                &self.package_name,
                &format!(
                    "target kind {:?} requires qualified native execution",
                    self.target.kind
                ),
            ));
        }
        if self.platform.is_some() {
            return Err(unsupported(
                &self.package_name,
                "configured target requires qualified native execution",
            ));
        }
        let crate_types_match = if script || integration {
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
    let links = matches!(unit.mode, Mode::Test)
        || unit
            .target
            .kind
            .iter()
            .any(|kind| matches!(kind.as_str(), "bin" | "cdylib" | "staticlib"));
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

    #[test]
    fn configured_linking_requires_declared_tools() {
        let mut graph: serde_json::Value = serde_json::from_slice(GRAPH).unwrap();
        graph["units"][0]["rustflags"] =
            serde_json::json!(["-C", "link-arg=-fuse-ld=experimental"]);
        for linker in [
            serde_json::json!("experimental-driver"),
            serde_json::Value::Null,
        ] {
            graph["units"][0]["linker"] = linker;
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
            assert!(render(CodeExecution::CompilerOnly).is_err());
            assert!(render(CodeExecution::DeclaredInputs).is_ok());
        }
    }
}
