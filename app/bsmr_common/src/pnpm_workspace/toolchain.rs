//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Selects and renders BSMR's finite, digest-pinned native Node toolchain catalog.

use std::fmt::Write;

use node_semver::Range;
use node_semver::Version;

use super::WorkspaceGraph;
use super::native_build::NativeTypeScriptBuildError;

pub(super) const TOOLCHAIN_TARGET: &str = "__bsmr_pnpm_toolchain";
const NODE_VERSION: &str = "26.5.1";

struct Archive {
    platform: &'static str,
    sha1: &'static str,
    sha256: &'static str,
    size: u64,
}

const NODE_ARCHIVES: [Archive; 4] = [
    Archive {
        platform: "darwin-arm64",
        sha1: "d6f4d14bb64d478c49a74742df7857c25387b971",
        sha256: "f4387df0b46556516d19abf2f2d6806481ac8368aa7f9d96bafed422a56a1d01",
        size: 57_178_864,
    },
    Archive {
        platform: "darwin-x64",
        sha1: "8c0c552a9b37997f89bf2251748b7b86101fb5d4",
        sha256: "077d5c936868dab19d21f77f1e71ce13697e80b3e86a399dcab238902a2ebf93",
        size: 58_481_270,
    },
    Archive {
        platform: "linux-arm64",
        sha1: "c20db109e035f21bd8725c61a736c59b3b3b26c2",
        sha256: "21194bbf41c18d9ec277545c4d14cce8597d57a9d9f494c323d8121a25de33e8",
        size: 61_392_027,
    },
    Archive {
        platform: "linux-x64",
        sha1: "e959809ae5c6720df0d038f7995b829d84a9c0a5",
        sha256: "2b07f09c218d473a26442bff5a90151f53f7b7c0a23bad244eda2c26303a2ba7",
        size: 61_546_562,
    },
];

struct PnpmArchive {
    identity: &'static str,
    version: &'static str,
    sha1: &'static str,
    sha256: &'static str,
    size: u64,
}

const PNPM_ARCHIVES: [PnpmArchive; 2] = [
    PnpmArchive {
        identity: "pnpm@10.30.3+sha512.c961d1e0a2d8e354ecaa5166b822516668b7f44cb5bd95122d590dd81922f606f5473b6d23ec4a5be05e7fcd18e8488d47d978bbe981872f1145d06e9a740017",
        version: "10.30.3",
        sha1: "e73903d314ca7c463712a4e6e3c696d4b0ffa4b0",
        sha256: "ff0a72140f6a6d66c0b284f6c9560aff605518e28c29aeac25fb262b74331588",
        size: 4_491_640,
    },
    PnpmArchive {
        identity: "pnpm@11.20.0+sha512.9a6f330a95b66446ea088faf1521405a8a01f07fde7124cc9958dfed52d4bb436737e65b08f85f37b46fcba375092558ac51262b816844b22f63406ed166bfee",
        version: "11.20.0",
        sha1: "bd611f7e6129c92783298b6eb823941eef37285b",
        sha256: "34e198cb1e43237517ecedfd31f9ae26a6c0a3e5366ce58a2d05f4b21fb5f19a",
        size: 8_784_864,
    },
];

/// Validates native requirements and renders the matching private toolchain.
pub(super) fn render_toolchain(
    graph: &WorkspaceGraph,
    output: &mut String,
) -> Result<(), NativeTypeScriptBuildError> {
    let toolchain = graph
        .node_toolchain()
        .ok_or(NativeTypeScriptBuildError::MissingToolchain)?;
    let requirement = Range::parse(toolchain.node_requirement()).map_err(|error| {
        NativeTypeScriptBuildError::InvalidNodeRequirement {
            requirement: toolchain.node_requirement().to_owned(),
            error: error.to_string(),
        }
    })?;
    let node = Version::parse(NODE_VERSION).expect("catalog version is valid");
    if !node.satisfies(&requirement) {
        return Err(NativeTypeScriptBuildError::UnsupportedNodeRequirement(
            toolchain.node_requirement().to_owned(),
        ));
    }
    let pnpm = PNPM_ARCHIVES
        .iter()
        .find(|archive| archive.identity == toolchain.package_manager())
        .ok_or_else(|| {
            NativeTypeScriptBuildError::UnsupportedPackageManager(
                toolchain.package_manager().to_owned(),
            )
        })?;
    render_toolchain_source(
        output,
        toolchain.node_requirement(),
        toolchain.package_manager(),
        pnpm,
    )
    .map_err(NativeTypeScriptBuildError::Render)
}

/// Writes validated toolchain rules without repeating formatting error plumbing.
fn render_toolchain_source(
    output: &mut String,
    node_requirement: &str,
    package_manager: &str,
    pnpm: &PnpmArchive,
) -> std::fmt::Result {
    writeln!(
        output,
        "load(\"@prelude//toolchains/pnpm:defs.bzl\", \"node_distribution\", \"pnpm_distribution\", \"pnpm_toolchain\")\n"
    )?;
    for archive in NODE_ARCHIVES {
        writeln!(output, "http_archive(")?;
        writeln!(
            output,
            "    name = {:?},",
            format!("__bsmr_node_{}", archive.platform)
        )?;
        writeln!(output, "    has_content_based_path = True,")?;
        writeln!(output, "    sha1 = {:?},", archive.sha1)?;
        writeln!(output, "    sha256 = {:?},", archive.sha256)?;
        writeln!(output, "    size_bytes = {},", archive.size)?;
        writeln!(
            output,
            "    strip_prefix = {:?},",
            format!("node-v{NODE_VERSION}-{}", archive.platform)
        )?;
        writeln!(
            output,
            "    urls = [{:?}],",
            format!(
                "https://nodejs.org/dist/v{NODE_VERSION}/node-v{NODE_VERSION}-{}.tar.gz",
                archive.platform
            )
        )?;
        writeln!(output, ")\n")?;
    }
    writeln!(output, "http_archive(")?;
    writeln!(output, "    name = \"__bsmr_pnpm_archive\",")?;
    writeln!(output, "    has_content_based_path = True,")?;
    writeln!(output, "    sha1 = {:?},", pnpm.sha1)?;
    writeln!(output, "    sha256 = {:?},", pnpm.sha256)?;
    writeln!(output, "    size_bytes = {},", pnpm.size)?;
    writeln!(
        output,
        "    urls = [{:?}],",
        format!(
            "https://registry.npmjs.org/pnpm/-/pnpm-{}.tgz",
            pnpm.version
        )
    )?;
    writeln!(output, ")\n")?;
    writeln!(output, "node_distribution(")?;
    writeln!(output, "    name = \"__bsmr_node_distribution\",")?;
    writeln!(output, "    node_requirement = {node_requirement:?},")?;
    writeln!(output, "    root = select({{")?;
    writeln!(
        output,
        "        \"config//os:linux\": select({{\"config//cpu:arm64\": \":__bsmr_node_linux-arm64\", \"config//cpu:x86_64\": \":__bsmr_node_linux-x64\"}}),"
    )?;
    writeln!(
        output,
        "        \"config//os:macos\": select({{\"config//cpu:arm64\": \":__bsmr_node_darwin-arm64\", \"config//cpu:x86_64\": \":__bsmr_node_darwin-x64\"}}),"
    )?;
    writeln!(output, "    }}),")?;
    writeln!(output, "    version = {NODE_VERSION:?},")?;
    writeln!(output, ")\n")?;
    writeln!(output, "pnpm_distribution(")?;
    writeln!(output, "    name = \"__bsmr_pnpm_distribution\",")?;
    writeln!(output, "    package_manager = {package_manager:?},")?;
    writeln!(output, "    root = \":__bsmr_pnpm_archive\",")?;
    writeln!(output, ")\n")?;
    writeln!(output, "pnpm_toolchain(")?;
    writeln!(output, "    name = {TOOLCHAIN_TARGET:?},")?;
    writeln!(output, "    node = \":__bsmr_node_distribution\",")?;
    writeln!(output, "    pnpm = \":__bsmr_pnpm_distribution\",")?;
    writeln!(output, ")\n")
}
