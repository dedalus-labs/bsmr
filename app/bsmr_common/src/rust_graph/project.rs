//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Opens an unchanged Cargo workspace with the bundled native build configuration.

use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use serde::Deserialize;

use super::toolchain::RustToolchain;
use super::unsupported;

/// These are the bundled native rules, without a configuration file in the checkout.
pub(crate) const CONFIG: &str = r"[project]
root = .
ignore = .git, target
[cells]
root = .
prelude = prelude
none = none
[cell_aliases]
config = prelude
ovr_config = prelude
upstream = none
toolchains = root
[external_cells]
prelude = bundled
[parser]
target_platform_detector_spec = target:root//...->prelude//platforms:default target:prelude//...->prelude//platforms:default
[build]
execution_platforms = prelude//platforms:default
";

#[derive(Deserialize)]
struct Located {
    /// Cargo's owning workspace manifest, not the nearest member manifest.
    root: PathBuf,
}

/// Ask the pinned Cargo to locate its workspace without invoking a build or writing project files.
pub(crate) fn root(from: &Path) -> bsmr_error::Result<PathBuf> {
    let source = from
        .ancestors()
        .find_map(|directory| {
            match std::fs::read_to_string(directory.join("rust-toolchain.toml")) {
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                result => Some(result),
            }
        })
        .ok_or_else(|| {
            unsupported(
                "workspace",
                "missing rust-toolchain.toml with an exact channel",
            )
        })??;
    let toolchain = RustToolchain::parse(&source)?;
    // Discovery also runs before the client's async runtime exists. A private thread
    // permits the same bounded operation from either synchronous or async callers.
    std::thread::scope(|scope| {
        scope
            .spawn(|| {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()?
                    .block_on(locate(from, &toolchain))
            })
            .join()
            .map_err(|_| bsmr_error::internal_error!("Cargo discovery worker panicked"))?
    })
}

/// Bound and reap the metadata-only subprocess, retaining diagnostics on failure.
async fn locate(from: &Path, toolchain: &RustToolchain) -> bsmr_error::Result<PathBuf> {
    let home = tempfile::tempdir()?;
    let output = home.path().join("stdout");
    let errors = home.path().join("stderr");
    let mut child = tokio::process::Command::new(toolchain.cargo())
        .args(["locate-project", "--workspace", "--frozen"])
        .current_dir(from)
        .env_clear()
        .env("CARGO_HOME", home.path())
        .stdout(std::fs::File::create(&output)?)
        .stderr(std::fs::File::create(&errors)?)
        .kill_on_drop(true)
        .spawn()?;
    let status = match tokio::time::timeout(Duration::from_secs(30), child.wait()).await {
        Ok(status) => status?,
        Err(_) => {
            child.kill().await?;
            return Err(unsupported("workspace", "Cargo discovery exceeded 30 seconds").into());
        }
    };
    if !status.success() {
        return Err(unsupported("workspace", &std::fs::read_to_string(errors)?).into());
    }
    let located: Located = serde_json::from_slice(&std::fs::read(output)?)?;
    let root = located
        .root
        .parent()
        .ok_or_else(|| unsupported("workspace", "manifest has no parent"))?;
    if !root.is_absolute()
        || !from.starts_with(root)
        || located
            .root
            .file_name()
            .is_none_or(|name| name != "Cargo.toml")
    {
        return Err(unsupported(
            "workspace",
            "Cargo returned a manifest outside the invocation ancestors",
        )
        .into());
    }
    Ok(root.to_owned())
}
