//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Defines the standard pyproject.toml subset consumed by the native frontend.

use std::collections::BTreeMap;

use serde::Deserialize;

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) struct Manifest {
    pub(super) build_system: Option<BuildSystem>,
    pub(super) project: Option<Project>,
    #[serde(default)]
    pub(super) tool: ToolConfiguration,
}

#[derive(Deserialize)]
pub(super) struct BuildSystem {
    #[serde(default)]
    pub(super) requires: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) struct Project {
    pub(super) name: String,
    #[serde(default)]
    pub(super) dependencies: Vec<String>,
    #[serde(default)]
    pub(super) optional_dependencies: BTreeMap<String, Vec<String>>,
    #[serde(default)]
    pub(super) dynamic: Vec<String>,
    pub(super) requires_python: Option<String>,
    #[serde(default)]
    pub(super) scripts: BTreeMap<String, String>,
}

#[derive(Default, Deserialize)]
pub(super) struct ToolConfiguration {
    #[serde(default)]
    pub(super) bsmr: BsmrConfiguration,
    #[serde(default)]
    pub(super) hatch: HatchConfiguration,
    pub(super) setuptools_scm: Option<BTreeMap<String, toml::Value>>,
    #[serde(default)]
    pub(super) uv: UvConfiguration,
    #[serde(rename = "uv-dynamic-versioning")]
    pub(super) uv_dynamic_versioning: Option<BTreeMap<String, toml::Value>>,
}

#[derive(Default, Deserialize)]
pub(super) struct BsmrConfiguration {
    #[serde(default)]
    pub(super) python: BsmrPythonConfiguration,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) struct BsmrPythonConfiguration {
    pub(super) test_command: Option<Vec<String>>,
}

#[derive(Default, Deserialize)]
pub(super) struct HatchConfiguration {
    pub(super) version: Option<HatchVersion>,
    pub(super) metadata: Option<HatchMetadata>,
}

#[derive(Deserialize)]
pub(super) struct HatchVersion {
    pub(super) source: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct HatchMetadata {
    pub(super) hooks: HatchMetadataHooks,
}

#[derive(Deserialize)]
pub(super) struct HatchMetadataHooks {
    #[serde(rename = "uv-dynamic-versioning")]
    pub(super) uv_dynamic_versioning: Option<HatchDynamicMetadata>,
}

#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) struct HatchDynamicMetadata {
    #[serde(default)]
    pub(super) dependencies: Vec<String>,
    #[serde(default)]
    pub(super) optional_dependencies: BTreeMap<String, Vec<String>>,
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(super) struct UvConfiguration {
    #[serde(default)]
    pub(super) cache_keys: Vec<UvCacheKey>,
    #[serde(default)]
    pub(super) config_settings: BTreeMap<String, BuildConfigSetting>,
    #[serde(default)]
    pub(super) config_settings_package: BTreeMap<String, BTreeMap<String, BuildConfigSetting>>,
    #[serde(default)]
    pub(super) extra_build_dependencies: BTreeMap<String, Vec<ExtraBuildDependency>>,
    #[serde(default)]
    pub(super) extra_build_variables: BTreeMap<String, BTreeMap<String, String>>,
    pub(super) package: Option<bool>,
    pub(super) workspace: Option<UvWorkspace>,
}

#[derive(Deserialize)]
pub(super) struct UvCacheKey {
    pub(super) git: Option<UvGitCacheKey>,
}

impl UvCacheKey {
    /// Returns whether this key declares Git state as dynamic build metadata.
    pub(super) fn uses_git(&self) -> bool {
        self.git.as_ref().is_some_and(|git| git.commit || git.tags)
    }
}

#[derive(Deserialize)]
pub(super) struct UvGitCacheKey {
    #[serde(default)]
    commit: bool,
    #[serde(default)]
    tags: bool,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub(super) enum ExtraBuildDependency {
    Requirement(String),
    Annotated {
        requirement: String,
        #[serde(rename = "match-runtime")]
        match_runtime: bool,
    },
}

impl ExtraBuildDependency {
    /// Returns the PEP 508 requirement selected during lock authoring.
    pub(super) fn requirement(&self) -> &str {
        match self {
            Self::Requirement(requirement) | Self::Annotated { requirement, .. } => requirement,
        }
    }

    /// Returns whether uv requires the build version to match the runtime installation.
    pub(super) fn match_runtime(&self) -> bool {
        matches!(
            self,
            Self::Annotated {
                match_runtime: true,
                ..
            }
        )
    }
}

#[derive(Deserialize)]
pub(super) struct UvWorkspace {
    #[serde(default)]
    pub(super) exclude: Vec<String>,
    #[serde(default)]
    pub(super) members: Vec<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub(super) enum BuildConfigSetting {
    One(String),
    Many(Vec<String>),
}

impl BuildConfigSetting {
    /// Returns PEP 517 values in their declared repetition order.
    pub(super) fn values(&self) -> &[String] {
        match self {
            Self::One(value) => std::slice::from_ref(value),
            Self::Many(values) => values,
        }
    }
}
