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

struct NodeRelease {
    version: &'static str,
    sha256: [&'static str; 4],
}

const NODE_PLATFORMS: [&str; 4] = ["darwin-arm64", "darwin-x64", "linux-arm64", "linux-x64"];
const NODE_RELEASES: [NodeRelease; 5] = [
    NodeRelease {
        version: "22.23.1",
        sha256: [
            "ef28d8fab2c0e4314522d4bb1b7173270aa3937e93b92cb7de79c112ac1fa953",
            "b8da981b8a0b1241b70249204916da76c63573ddf5814dbd2d1e41069105cb81",
            "543fa39e57d4c07855939459a323f4deb9a79dd1bb45e6e99458b0f2de10db8d",
            "7a8cb04b4a1df4eaf432125324b81b29a088e73570a23259a8de1c65d07fc129",
        ],
    },
    NodeRelease {
        version: "24.18.0",
        sha256: [
            "e1a97e14c99c803e96c7339403282ea05a499c32f8d83defe9ef5ec66f979ed1",
            "dfd0dbd3e721503434df7b7205e719f61b3a3a31b2bcf9729b8b91fea240f080",
            "6b4484c2190274175df9aa8f28e2d758a819cb1c1fe6ab481e2f95b463ab8508",
            "783130984963db7ba9cbd01089eaf2c2efb055c7c1693c943174b967b3050cb8",
        ],
    },
    NodeRelease {
        version: "24.19.0",
        sha256: [
            "8294b7aa9b03997481c06babf1e8b270c859358f27da57a11509afe537ac381d",
            "d1b5e999db158c62fe8f7267a4476b035d8bd93b1a605bac24a3f0dd166e3316",
            "d28c8a5bf0a808f0ed434a1dce8c54ae98f0371c0bd86ac58abc613f73e6643f",
            "f625d97cd707df4ff96254916fbc5ff014f09c09effe5a1e0ca8f6d41a8789d4",
        ],
    },
    NodeRelease {
        version: "26.5.1",
        sha256: [
            "f4387df0b46556516d19abf2f2d6806481ac8368aa7f9d96bafed422a56a1d01",
            "077d5c936868dab19d21f77f1e71ce13697e80b3e86a399dcab238902a2ebf93",
            "21194bbf41c18d9ec277545c4d14cce8597d57a9d9f494c323d8121a25de33e8",
            "2b07f09c218d473a26442bff5a90151f53f7b7c0a23bad244eda2c26303a2ba7",
        ],
    },
    NodeRelease {
        version: "26.7.0",
        sha256: [
            "7ee659a7768e641bbfd5360940660b8e8fd0052f77488f365562bac522fc15d4",
            "f279d1ed28ce57f7788bf23435d2ad7fdd7438904ad5c4d8a1081a7cde3d4b96",
            "925aa6157dd37542d0d7f2e28b7bf61e7b39284411210b0498bc3788db4aef68",
            "bd6b6c31e377bad9ad579bed72e5bc11f4c879ac9452ad51d30e646ea3d828df",
        ],
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
    let node = select_node_release(toolchain, &requirement)?;
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
        node,
        pnpm,
    )
    .map_err(NativeTypeScriptBuildError::Render)
}

/// Selects pnpm's exact runtime or the newest catalog entry satisfying `engines.node`.
fn select_node_release(
    toolchain: &super::NodeWorkspaceToolchain,
    requirement: &Range,
) -> Result<&'static NodeRelease, NativeTypeScriptBuildError> {
    if let Some(version) = toolchain.runtime_version() {
        let parsed = Version::parse(version).map_err(|error| {
            NativeTypeScriptBuildError::InvalidNodeRuntime {
                version: version.to_owned(),
                error: error.to_string(),
            }
        })?;
        if !parsed.satisfies(requirement) {
            return Err(NativeTypeScriptBuildError::IncompatibleNodeRuntime {
                version: version.to_owned(),
                requirement: toolchain.node_requirement().to_owned(),
            });
        }
        return NODE_RELEASES
            .iter()
            .find(|release| release.version == version)
            .ok_or_else(|| NativeTypeScriptBuildError::UnsupportedNodeRuntime {
                version: version.to_owned(),
                available: catalog_versions(),
            });
    }
    NODE_RELEASES
        .iter()
        .rev()
        .find(|release| {
            Version::parse(release.version)
                .expect("catalog version is valid")
                .satisfies(requirement)
        })
        .ok_or_else(|| NativeTypeScriptBuildError::UnsupportedNodeRequirement {
            requirement: toolchain.node_requirement().to_owned(),
            available: catalog_versions(),
        })
}

/// Writes validated toolchain rules without repeating formatting error plumbing.
fn render_toolchain_source(
    output: &mut String,
    node_requirement: &str,
    package_manager: &str,
    node: &NodeRelease,
    pnpm: &PnpmArchive,
) -> std::fmt::Result {
    writeln!(
        output,
        "load(\"@prelude//toolchains/pnpm:defs.bzl\", \"node_distribution\", \"pnpm_distribution\", \"pnpm_toolchain\")\n"
    )?;
    for (platform, sha256) in NODE_PLATFORMS.iter().zip(node.sha256) {
        writeln!(output, "http_archive(")?;
        writeln!(
            output,
            "    name = {:?},",
            format!("__bsmr_node_{platform}")
        )?;
        writeln!(output, "    has_content_based_path = True,")?;
        writeln!(output, "    sha256 = {sha256:?},")?;
        writeln!(
            output,
            "    strip_prefix = {:?},",
            format!("node-v{}-{platform}", node.version)
        )?;
        writeln!(
            output,
            "    urls = [{:?}],",
            format!(
                "https://nodejs.org/dist/v{0}/node-v{0}-{1}.tar.gz",
                node.version, platform
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
    writeln!(output, "    version = {:?},", node.version)?;
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

/// Lists catalog versions in deterministic selection order for diagnostics.
fn catalog_versions() -> String {
    NODE_RELEASES
        .iter()
        .map(|release| release.version)
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use node_semver::Range;

    use super::select_node_release;
    use crate::pnpm_workspace::NodeWorkspaceToolchain;

    fn selected(requirement: &str, runtime: Option<&str>) -> Result<&'static str, String> {
        let toolchain = NodeWorkspaceToolchain {
            node_requirement: requirement.to_owned(),
            package_manager: "pnpm@test".to_owned(),
            runtime_version: runtime.map(str::to_owned),
        };
        let requirement = Range::parse(requirement).unwrap();
        select_node_release(&toolchain, &requirement)
            .map(|release| release.version)
            .map_err(|error| error.to_string())
    }

    #[test]
    fn invariant_catalog_selects_newest_compatible_runtime() {
        for (requirement, expected) in [
            ("^22.0.0", "22.23.1"),
            ("24.18.0", "24.18.0"),
            ("^24.0.0", "24.19.0"),
            (">=24.0.0", "26.7.0"),
        ] {
            assert_eq!(selected(requirement, None).unwrap(), expected);
        }
    }

    #[test]
    fn invariant_pnpm_runtime_pin_is_exact_and_engine_compatible() {
        assert_eq!(selected(">=22.0.0", Some("24.18.0")).unwrap(), "24.18.0");
        assert_eq!(
            selected("^24.0.0", Some("22.23.1")).unwrap_err(),
            "pnpm useNodeVersion `22.23.1` does not satisfy engines.node `^24.0.0`"
        );
    }
}
