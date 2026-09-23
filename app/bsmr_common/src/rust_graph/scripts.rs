//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Lowers a configured build-script execution through the inherited native runner.

use super::Renderer;
use super::RustGraphError;
use super::Unit;
use super::json;
use super::unsupported;
use crate::rust_graph::units::Mode;

impl Renderer<'_> {
    /// Bind the script binary, source tree and target configuration to one execution.
    pub(super) fn script(&self, unit: &Unit, index: usize) -> Result<String, RustGraphError> {
        let [dependency] = unit.dependencies.as_slice() else {
            return Err(unsupported(
                &unit.package_name,
                "build-script native metadata dependencies",
            ));
        };
        let executable = &self.graph.units[dependency.index];
        if !matches!(executable.mode, Mode::Build)
            || executable.target.kind != ["custom-build"]
            || executable.package_id != unit.package_id
        {
            return Err(unsupported(
                &unit.package_name,
                "build-script compiler ownership",
            ));
        }
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
             buildscript_run(name = \"unit_{index}\", buildscript_rule = \":unit_{binary}\", package_name = {package}, version = {version}, manifest_dir = {sources}, features = {features}, literal_env = {environment}, rustc_cfg = \":cfg_{index}\", rustc_host_tuple = \":host_{index}\", _rust_toolchain = {toolchain}, rustc_link_lib = True, rustc_link_search = True, visibility = [\"PUBLIC\"])\n",
            toolchain = json(self.toolchain)?,
            cfgs = json(&cfgs)?,
            host = json(&[&environment["HOST"]])?,
            binary = dependency.index,
            package = json(&unit.package_name)?,
            version = json(&unit.package_version)?,
            sources = json(&self.sources[unit.package_id.as_str()])?,
            features = json(&unit.features)?,
            environment = json(&environment)?,
        ))
    }
}
