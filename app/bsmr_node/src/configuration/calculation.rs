//===----------------------------------------------------------------------===//
// Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc
// Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use allocative::Allocative;
use async_trait::async_trait;
use bsmr_core::cells::name::CellName;
use bsmr_core::configuration::data::ConfigurationData;
use bsmr_core::target::label::label::TargetLabel;
use bsmr_util::late_binding::LateBinding;
use derive_more::Display;
use dice::DiceComputations;
use dupe::Dupe;

#[async_trait]
pub trait ConfigurationCalculationDyn: Send + Sync + 'static {
    async fn get_platform_configuration(
        &self,
        dice: &mut DiceComputations<'_>,
        target: &TargetLabel,
    ) -> bsmr_error::Result<ConfigurationData>;
}

/// For config_settings that need to be resolved when producing a ResolvedConfiguration, the bsmrconfig values are looked up in
/// the cell that the configuration is resolving in. This means that for selects that appear in a target, the config_settings in the keys
/// would resolve based on the bsmrconfigs from that target's cell.
///
/// This is subtle, non-obvious and possibly unintuitive, so we introduce a newtype here just to make it clearer in the places we are
/// using or passing around a CellName for this purpose.
#[derive(
    Clone,
    Dupe,
    Copy,
    Debug,
    Display,
    Hash,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Allocative,
    pagable::Pagable
)]
pub struct CellNameForConfigurationResolution(pub CellName);

pub static CONFIGURATION_CALCULATION: LateBinding<&'static dyn ConfigurationCalculationDyn> =
    LateBinding::new("CONFIGURATION_CALCULATION");
