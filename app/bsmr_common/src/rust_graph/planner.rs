//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Invokes the installed Cargo planner against a tracked, frozen workspace snapshot.

use std::path::Path;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use serde_json::json;
use sha2::Digest;
use sha2::Sha256;
use tokio::io::AsyncWriteExt;

use super::RustGraphError;
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
            let directory = executable.parent().expect("executable has parent");
            return Err(RustGraphError::MissingPlanner(directory.to_owned()).into());
        }
        let home = crate::invocation_roots::home_bsmr_dir()?.as_path();
        let cargo_home = home.join("cargo/0.98.0");
        let target_directory = planner_target_directory(home, self.root);
        std::fs::create_dir_all(&cargo_home)?;
        std::fs::create_dir_all(&target_directory)?;
        let request = self.request(entries, packages, &cargo_home, &target_directory);
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
        target_directory: &Path,
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
            "target_directory": target_directory,
        })
    }
}

/// Derives a workspace-scoped planner target directory inside the user state root.
fn planner_target_directory(home_bsmr_dir: &Path, root: &Path) -> PathBuf {
    let mut hasher = Sha256::new();
    hasher.update(root.as_os_str().as_encoded_bytes());
    let digest = hex::encode(hasher.finalize());
    home_bsmr_dir.join("cargo-planner").join(&digest[..32])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planner_target_directory_lives_under_user_state_root() {
        let home = Path::new("/home/user/.bsmr");
        let root_a = Path::new("/work/repo-a");
        let root_b = Path::new("/work/repo-b");

        let dir_a = planner_target_directory(home, root_a);
        let dir_b = planner_target_directory(home, root_b);

        assert!(dir_a.starts_with(home.join("cargo-planner")));
        assert!(!dir_a.starts_with(root_a));
        assert_ne!(dir_a, home.join("cargo/0.98.0"));
        assert_ne!(dir_a, dir_b);
        assert_eq!(dir_a, planner_target_directory(home, root_a));
    }
}
