//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Selects pinned Rust distributions from the project's standard toolchain file.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::Deserialize;

use super::unsupported;

#[derive(Deserialize)]
struct ToolchainFile {
    toolchain: Toolchain,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Toolchain {
    channel: String,
    profile: Option<String>,
    #[serde(default)]
    components: Vec<String>,
    #[serde(default)]
    targets: Vec<String>,
}

#[derive(Deserialize)]
struct Archive {
    url: String,
    sha256: String,
}

/// The metadata resolver and the declared compiler distribution share one release.
pub(super) struct RustToolchain {
    pub cargo: PathBuf,
    pub rustc: PathBuf,
    pub rules: String,
}

impl RustToolchain {
    /// Require a supported exact pin and an installed Cargo for offline resolution.
    pub fn parse(source: &str) -> bsmr_error::Result<Self> {
        let ToolchainFile { toolchain } = toml::from_str(source)?;
        let host = match (std::env::consts::OS, std::env::consts::ARCH) {
            ("linux", "aarch64") => "aarch64-unknown-linux-gnu",
            ("linux", "x86_64") => "x86_64-unknown-linux-gnu",
            ("macos", "aarch64") => "aarch64-apple-darwin",
            ("macos", "x86_64") => "x86_64-apple-darwin",
            _ => return Err(unsupported("toolchain", "this execution platform").into()),
        };
        if !toolchain.targets.iter().all(|target| target == host)
            || !toolchain
                .components
                .iter()
                .all(|c| ["rustfmt", "clippy"].contains(&c.as_str()))
            || toolchain
                .profile
                .as_deref()
                .is_some_and(|p| !["minimal", "default"].contains(&p))
        {
            return Err(unsupported("toolchain", "custom components or cross compilation").into());
        }
        let releases: BTreeMap<String, BTreeMap<String, BTreeMap<String, Archive>>> =
            serde_json::from_str(include_str!("releases.json"))?;
        let archives = releases
            .get(&toolchain.channel)
            .and_then(|r| r.get(host))
            .ok_or_else(|| {
                unsupported(
                    "toolchain",
                    &format!(
                        "channel `{}`; supported pins: 1.97.1, nightly-2026-04-11",
                        toolchain.channel
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
            rules.push_str(&format!("http_archive(name = \"__bsmr_{component}\", urls = [{url:?}], sha256 = {hash:?}, strip_prefix = \"{prefix}/{directory}\", has_content_based_path = True)\n", url=archive.url, hash=archive.sha256));
        }
        let nightly = if toolchain.channel.starts_with("nightly-") {
            "True"
        } else {
            "False"
        };
        rules.push_str(&format!("native_rust_toolchain(name = \"__bsmr_rust\", compiler = \":__bsmr_rustc\", clippy = \":__bsmr_clippy-preview\", standard_library = \":__bsmr_rust-std\", triple = {host:?}, nightly_features = {nightly}, visibility = [\"PUBLIC\"])\n"));
        rules.push_str(&format!("native_rust_tools(triple = {host:?})\n"));
        Ok(Self {
            cargo,
            rustc,
            rules,
        })
    }
}
