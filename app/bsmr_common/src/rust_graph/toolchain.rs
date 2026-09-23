//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Selects pinned Rust distributions from the project's standard toolchain file.

use std::collections::BTreeMap;
use std::num::NonZeroU64;
use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;

use super::unsupported;

#[derive(Deserialize)]
struct ToolchainFile {
    /// The standard rustup selection in rust-toolchain.toml.
    toolchain: Toolchain,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Toolchain {
    /// Exact release key in the checked-in distribution catalog.
    channel: String,
    /// Only minimal and default installations are supported.
    profile: Option<String>,
    /// Additional tools, restricted to rustfmt and clippy.
    #[serde(default)]
    components: Vec<String>,
    /// Requested targets must match the execution host.
    #[serde(default)]
    targets: Vec<String>,
}

#[derive(Deserialize)]
struct Archive {
    /// Immutable compiler component archive.
    url: String,
    /// Expected digest before extraction.
    sha256: String,
    /// Pinned archive length avoids network discovery before cached materialization.
    size_bytes: NonZeroU64,
}

/// The metadata resolver and the declared compiler distribution share one release.
pub(super) struct RustToolchain {
    /// Installed resolver for this exact release.
    cargo: PathBuf,
    /// Matching compiler used by Cargo to inspect its host.
    rustc: PathBuf,
    /// Native rules that acquire and select the same compiler.
    rules: String,
}

impl RustToolchain {
    /// Require a supported exact pin and an installed Cargo for offline resolution.
    pub fn parse(source: &str) -> bsmr_error::Result<Self> {
        let ToolchainFile { toolchain } = toml::from_str(source)?;
        let host = toolchain.host()?;
        let releases: BTreeMap<String, BTreeMap<String, BTreeMap<String, Archive>>> =
            serde_json::from_str(include_str!("releases.json"))?;
        let archives = releases
            .get(&toolchain.channel)
            .and_then(|r| r.get(host))
            .ok_or_else(|| {
                unsupported(
                    "toolchain",
                    &format!(
                        "channel `{}`; supported pins: {}",
                        toolchain.channel,
                        releases.keys().cloned().collect::<Vec<_>>().join(", "),
                    ),
                )
            })?;
        let home = std::env::var_os("RUSTUP_HOME")
            .map(PathBuf::from)
            .or_else(|| dirs::home_dir().map(|p| p.join(".rustup")))
            .ok_or_else(|| unsupported("toolchain", "missing rustup home"))?;
        let bin = home
            .join("toolchains")
            .join(format!("{}-{host}", toolchain.channel))
            .join("bin");
        let cargo = bin.join("cargo");
        let rustc = bin.join("rustc");
        if !cargo.is_file() || !rustc.is_file() {
            return Err(unsupported(
                "toolchain",
                &format!(
                    "missing resolver; run `rustup toolchain install {} --profile minimal`",
                    toolchain.channel
                ),
            )
            .into());
        }
        let rules = toolchain.rules(host, archives);
        Ok(Self {
            cargo,
            rustc,
            rules,
        })
    }

    /// Return the installed resolver paired with the admitted compiler distribution.
    pub fn cargo(&self) -> &Path {
        &self.cargo
    }

    /// Return the matching compiler used for Cargo's capability probes.
    pub fn rustc(&self) -> &Path {
        &self.rustc
    }

    /// Return native acquisition rules for this exact compiler and standard library.
    pub fn rules(&self) -> &str {
        &self.rules
    }
}

impl Toolchain {
    /// Admit native execution without silently dropping requested targets or components.
    fn host(&self) -> bsmr_error::Result<&'static str> {
        let host = match (std::env::consts::OS, std::env::consts::ARCH) {
            ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
            ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
            ("macos", "aarch64") => "aarch64-apple-darwin",
            ("macos", "x86_64") => "x86_64-apple-darwin",
            _ => return Err(unsupported("toolchain", "this execution platform").into()),
        };
        if !self.targets.iter().all(|target| target == host)
            || !self
                .components
                .iter()
                .all(|c| ["rustfmt", "clippy"].contains(&c.as_str()))
            || self
                .profile
                .as_deref()
                .is_some_and(|p| !["minimal", "default"].contains(&p))
        {
            return Err(unsupported("toolchain", "custom components or cross compilation").into());
        }
        Ok(host)
    }

    /// Render pinned archives and their single native toolchain.
    fn rules(&self, host: &str, archives: &BTreeMap<String, Archive>) -> String {
        let mut rules = String::from(
            "load(\"@prelude//rust/native:toolchain.bzl\", \"native_rust_toolchain\", \"native_rust_tools\")\n",
        );
        for (component, archive) in archives {
            let filename = archive
                .url
                .rsplit('/')
                .next()
                .expect("catalog URL has a filename");
            let prefix = filename
                .strip_suffix(".tar.gz")
                .expect("catalog archive is gzip");
            let directory = if component != "rust-std" {
                component.clone()
            } else {
                format!("rust-std-{host}")
            };
            rules.push_str(&format!("http_archive(name = \"__bsmr_{component}\", urls = [{url:?}], sha256 = {hash:?}, size_bytes = {size}, strip_prefix = \"{prefix}/{directory}\", has_content_based_path = True)\n", url=archive.url, hash=archive.sha256, size=archive.size_bytes));
        }
        let nightly = if self.channel.starts_with("nightly-") {
            "True"
        } else {
            "False"
        };
        rules.push_str(&format!("native_rust_toolchain(name = \"__bsmr_rust\", compiler = \":__bsmr_rustc\", clippy = \":__bsmr_clippy-preview\", standard_library = \":__bsmr_rust-std\", triple = {host:?}, nightly_features = {nightly}, visibility = [\"PUBLIC\"])\n"));
        rules.push_str(&format!("native_rust_tools(triple = {host:?})\n"));
        rules
    }
}
