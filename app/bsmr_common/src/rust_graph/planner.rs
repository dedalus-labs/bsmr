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

use super::entry::Entry;
use super::entry::Mode;
use super::entry::Target;
use super::toolchain::RustToolchain;
use super::unsupported;

/// Resolve one selected entrypoint without allowing Cargo to compile source code.
pub(super) async fn resolve(
    root: &Path,
    entry: &Entry,
    package: &str,
    toolchain: &RustToolchain,
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
    let target_filter = match &entry.target {
        Target::Lib(_) => json!({"kind": "library"}),
        Target::Bin(name) => json!({"kind": "binary", "name": name}),
    };
    let profile = match entry.mode {
        Mode::Build => "dev",
        Mode::Test => "test",
    };
    let request = json!({
        "manifest": root.join("Cargo.toml"), "package": package,
        "mode": entry.mode.as_str(), "target_filter": target_filter,
        "source_policy": "acquire-locked", "features": [],
        "default_features": true, "all_features": false, "target": null,
        "profile": profile, "cargo_home": cargo_home, "rustc": toolchain.rustc(),
        "target_directory": root.join(".bsmr-planner"),
    });
    let bytes = serde_json::to_vec(&request)?;
    let lock = std::fs::read(root.join("Cargo.lock"))?;
    let output = tokio::time::timeout(Duration::from_secs(300), async {
        let mut child = tokio::process::Command::new(executable)
            .current_dir(root)
            .env_clear()
            .env("CARGO_HOME", &cargo_home)
            .env(
                "PATH",
                toolchain.cargo().parent().expect("Cargo has parent"),
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
    .map_err(|_| unsupported(package, "configured Cargo planning exceeded 300 seconds"))??;
    if !output.status.success() {
        return Err(unsupported(package, &String::from_utf8_lossy(&output.stderr)).into());
    }
    if std::fs::read(root.join("Cargo.lock"))? != lock {
        return Err(unsupported(package, "Cargo.lock changed during frozen planning").into());
    }
    Ok(output.stdout)
}
