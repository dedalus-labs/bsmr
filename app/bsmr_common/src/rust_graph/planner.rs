//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Invokes the installed Cargo planner against a tracked, frozen workspace snapshot.

use std::path::Path;
use std::process::Stdio;
use std::time::Duration;

use serde_json::json;
use tokio::io::AsyncWriteExt;

use super::entry::Requested;
use super::entry::Target;
use super::selection::Selection;
use super::toolchain::RustToolchain;
use super::unsupported;

/// Holds one captured workspace and its immutable compiler and build selection.
pub(super) struct Planner<'a> {
    /// Tracked manifest snapshot held for the full request lifetime.
    root: &'a Path,
    /// Resolver and compiler admitted as one toolchain.
    toolchain: &'a RustToolchain,
    /// Tracked configuration for this build or test graph.
    selection: Selection,
}

impl<'a> Planner<'a> {
    /// Bind planning to one snapshot without permitting later selection mutation.
    pub fn new(root: &'a Path, toolchain: &'a RustToolchain, selection: Selection) -> Self {
        Self {
            root,
            toolchain,
            selection,
        }
    }

    /// Resolve one entrypoint without allowing Cargo to compile source code.
    pub async fn resolve(
        &self,
        entries: &[Requested],
        packages: &[String],
    ) -> bsmr_error::Result<Vec<u8>> {
        let executable = std::env::current_exe()?.with_file_name("bsmr-cargo");
        if !executable.is_file() {
            return Err(unsupported(
                "planner",
                "missing bsmr-cargo beside the installed bsmr binary",
            )
            .into());
        }
        let cargo_home = dirs::cache_dir()
            .ok_or_else(|| unsupported("planner", "missing cache directory"))?
            .join("bsmr/cargo/0.98.0");
        std::fs::create_dir_all(&cargo_home)?;
        let request = self.request(entries, packages, &cargo_home);
        let bytes = serde_json::to_vec(&request)?;
        let lock = std::fs::read(self.root.join("Cargo.lock"))?;
        let output = tokio::time::timeout(Duration::from_secs(300), async {
            let mut child = tokio::process::Command::new(executable)
                .current_dir(self.root)
                .env_clear()
                .env("CARGO_HOME", &cargo_home)
                .env(
                    "PATH",
                    self.toolchain.cargo().parent().expect("Cargo has parent"),
                )
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .kill_on_drop(true)
                .spawn()?;
            child
                .stdin
                .take()
                .expect("piped planner stdin")
                .write_all(&bytes)
                .await?;
            child.wait_with_output().await
        })
        .await
        .map_err(|_| unsupported("planner", "configured Cargo planning exceeded 300 seconds"))??;
        if !output.status.success() {
            return Err(unsupported("planner", &String::from_utf8_lossy(&output.stderr)).into());
        }
        if std::fs::read(self.root.join("Cargo.lock"))? != lock {
            return Err(unsupported("planner", "Cargo.lock changed during frozen planning").into());
        }
        Ok(output.stdout)
    }

    /// Preserve Cargo's request fields while sourcing choices from tracked configuration.
    fn request(
        &self,
        entries: &[Requested],
        packages: &[String],
        cargo_home: &Path,
    ) -> serde_json::Value {
        let targets: Vec<_> = entries
            .iter()
            .zip(packages)
            .map(|(request, package)| {
                let entry = &request.entry;
                let kind = match entry.target {
                    Target::Lib(_) => "library",
                    Target::Bin(_) => "binary",
                    Target::Test(_) => "integration-test",
                };
                json!({"package": package, "kind": kind, "name": entry.target.name(), "origin": request.origin})
            })
            .collect();
        json!({
            "manifest": self.root.join("Cargo.toml"), "packages": packages.iter().collect::<std::collections::BTreeSet<_>>(),
            "mode": entries[0].entry.mode.as_str(), "target_filter": {"kind": "targets", "targets": targets},
            "source_policy": "acquire-locked", "features": self.selection.features(),
            "default_features": self.selection.default_features(),
            "all_features": self.selection.all_features(), "target": null,
            "profile": self.selection.profile(), "cargo_home": cargo_home,
            "rustc": self.toolchain.rustc(),
            "target_directory": self.root.join(".bsmr-planner"),
        })
    }
}
