//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Reads Cargo selections through the engine's tracked configuration interface.

use bsmr_core::cells::name::CellName;
use dice::DiceComputations;

use super::entry::Mode;
use crate::legacy_configs::dice::HasLegacyConfigs;
use crate::legacy_configs::key::BsmrconfigKeyRef;

/// Immutable request inputs. Cargo owns profile inheritance and feature-name validation.
pub(super) struct Selection {
    /// Requested Cargo profile, with the operation's default applied once.
    profile: String,
    /// Cargo CLI feature expressions, preserved for Cargo to parse.
    features: Vec<String>,
    /// Whether Cargo may activate each selected package's default features.
    default_features: bool,
    /// Whether Cargo activates all features of the selected packages.
    all_features: bool,
}

impl Selection {
    /// Track only these properties so changes invalidate the selected Cargo graph.
    pub async fn read(
        ctx: &mut DiceComputations<'_>,
        cell: CellName,
        mode: Mode,
    ) -> bsmr_error::Result<Self> {
        let key = |property| BsmrconfigKeyRef {
            section: "rust",
            property,
        };
        let profile = ctx.get_legacy_config_property(cell, key("profile")).await?;
        let features = ctx
            .get_legacy_config_property(cell, key("features"))
            .await?;
        let default_features = ctx
            .parse_legacy_config_property::<bool>(cell, key("default_features"))
            .await?;
        let all_features = ctx
            .parse_legacy_config_property::<bool>(cell, key("all_features"))
            .await?;
        Ok(Self {
            profile: profile
                .map(|value| value.to_string())
                .unwrap_or_else(|| match mode {
                    Mode::Build => "dev".to_owned(),
                    Mode::Test => "test".to_owned(),
                }),
            features: features
                .into_iter()
                .map(|value| value.to_string())
                .collect(),
            default_features: default_features.unwrap_or(true),
            all_features: all_features.unwrap_or(false),
        })
    }

    /// Return the effective Cargo profile name.
    pub fn profile(&self) -> &str {
        &self.profile
    }

    /// Return feature expressions without duplicating Cargo's parser.
    pub fn features(&self) -> &[String] {
        &self.features
    }

    /// Return whether default features participate in resolution.
    pub fn default_features(&self) -> bool {
        self.default_features
    }

    /// Return whether all package features participate in resolution.
    pub fn all_features(&self) -> bool {
        self.all_features
    }
}
