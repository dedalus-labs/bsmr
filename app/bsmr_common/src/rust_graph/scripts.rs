//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Lowers a configured build-script execution through the inherited native runner.

use std::collections::BTreeMap;

use super::Renderer;
use super::RustGraphError;
use super::Unit;
use super::json;
use super::unsupported;
use crate::rust_graph::units::Mode;

/// Cargo selects the script executable and the direct native metadata producers.
struct ScriptDependencies<'a> {
    /// This package's compiled build script.
    compiler: usize,
    /// Native library namespaces mapped to their producing script actions.
    metadata: BTreeMap<&'a str, String>,
}

impl Renderer<'_> {
    /// Bind the script binary, source tree and target configuration to one execution.
    pub(super) fn script(&self, unit: &Unit, index: usize) -> Result<String, RustGraphError> {
        let dependencies = self.script_dependencies(unit)?;
        let mut environment = unit.package_environment.clone();
        for key in [
            "OPT_LEVEL",
            "DEBUG",
            "PROFILE",
            "HOST",
            "TARGET",
            "CARGO_CFG_TARGET_ARCH",
        ] {
            if !environment.contains_key(key) {
                return Err(unsupported(
                    &unit.package_name,
                    "build-script profile environment requires the matching planner",
                ));
            }
        }
        environment.insert("NUM_JOBS".into(), "1".into());
        if let Some(linker) = &unit.linker {
            environment.insert("RUSTC_LINKER".into(), linker.to_string_lossy().into_owned());
        }
        environment.insert(
            "CARGO_ENCODED_RUSTFLAGS".into(),
            unit.rustflags.join("\x1f"),
        );
        let mut cfgs = Vec::new();
        environment.retain(|key, value| {
            let Some(key) = key.strip_prefix("CARGO_CFG_") else {
                return true;
            };
            cfgs.push(format!("{}=\"{}\"", key.to_lowercase(), value));
            false
        });
        Ok(format!(
            "load(\"@prelude//rust:cargo_buildscript.bzl\", \"buildscript_run\")\n\
             write_file(name = \"cfg_{index}\", out = \"rustc.cfg\", content = {cfgs}, newline = \"unix\")\n\
             write_file(name = \"host_{index}\", out = \"rustc.host\", content = {host}, newline = \"unix\")\n\
             buildscript_run(name = \"unit_{index}\", buildscript_rule = \":unit_{binary}\", metadata_deps = {metadata}, package_name = {package}, version = {version}, manifest_dir = {sources}, features = {features}, literal_env = {environment}, rustc_cfg = \":cfg_{index}\", rustc_host_tuple = \":host_{index}\", _rust_toolchain = {toolchain}, rustc_link_lib = True, rustc_link_search = True, visibility = [\"PUBLIC\"])\n",
            toolchain = json(self.toolchain)?,
            cfgs = json(&cfgs)?,
            host = json(&[&environment["HOST"]])?,
            binary = dependencies.compiler,
            metadata = json(&dependencies.metadata)?,
            package = json(&unit.package_name)?,
            version = json(&unit.package_version)?,
            sources = json(&self.sources[unit.package_id.as_str()])?,
            features = json(&unit.features)?,
            environment = json(&environment)?,
        ))
    }

    /// Keep metadata on Cargo's direct script edges without traversing library dependencies.
    fn script_dependencies(&self, unit: &Unit) -> Result<ScriptDependencies<'_>, RustGraphError> {
        let mut compiler = None;
        let mut metadata = BTreeMap::new();
        for dependency in &unit.dependencies {
            let target = &self.graph.units[dependency.index];
            if matches!(target.mode, Mode::Build)
                && target.target.kind == ["custom-build"]
                && target.package_id == unit.package_id
                && compiler.replace(dependency.index).is_none()
            {
                continue;
            }
            if matches!(target.mode, Mode::RunCustomBuild)
                && target.package_id != unit.package_id
                && let Some(links) = target.package_links.as_deref()
                && metadata
                    .insert(links, format!(":unit_{}", dependency.index))
                    .is_none()
            {
                continue;
            }
            return Err(unsupported(
                &unit.package_name,
                "build-script dependency ownership",
            ));
        }
        Ok(ScriptDependencies {
            compiler: compiler
                .ok_or_else(|| unsupported(&unit.package_name, "missing build-script compiler"))?,
            metadata,
        })
    }
}
