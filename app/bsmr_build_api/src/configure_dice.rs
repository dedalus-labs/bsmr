/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::sync::Arc;

use bsmr_common::dice::cells::SetCellResolver;
use bsmr_common::dice::data::SetIoProvider;
use bsmr_common::io::IoProvider;
use bsmr_common::legacy_configs::configs::LegacyBsmrConfig;
use bsmr_common::legacy_configs::dice::SetLegacyConfigs;
use bsmr_common::legacy_configs::key::BsmrconfigKeyRef;
use bsmr_common::tenting::SetTentingAclProvider;
use bsmr_common::tenting::TentingAclProvider;
use bsmr_core::rollout_percentage::RolloutPercentage;
use bsmr_execute::digest_config::DigestConfig;
use bsmr_execute::digest_config::SetDigestConfig;
use dice::DetectCycles;
use dice::Dice;
use dice::DiceStorage;

use crate::actions::execute::dice_data::SetInvalidationTrackingConfig;
use crate::build::detailed_aggregated_metrics::dice::SetDetailedAggregatedMetricsHandle;
use crate::build::detailed_aggregated_metrics::events::DetailedAggregatedMetricsHandle;

/// Utility to configure the dice globals.
/// One place to not forget to initialize something in all places.
pub async fn configure_dice_for_buck(
    io: Arc<dyn IoProvider>,
    digest_config: DigestConfig,
    root_config: Option<&LegacyBsmrConfig>,
    detect_cycles: Option<DetectCycles>,
    tenting_acl_provider: Option<Arc<dyn TentingAclProvider>>,
) -> bsmr_error::Result<Arc<Dice>> {
    let detect_cycles = detect_cycles.map_or_else(
        || {
            root_config
                .and_then(|c| {
                    c.parse::<DetectCycles>(BsmrconfigKeyRef {
                        section: "bsmr",
                        property: "detect_cycles",
                    })
                    .transpose()
                })
                .unwrap_or(Ok(DetectCycles::Enabled))
        },
        Ok,
    )?;

    let mut dice = Dice::builder();
    dice.set_io_provider(io);
    dice.set_digest_config(digest_config);
    dice.set_tenting_acl_provider(tenting_acl_provider);
    let invalidation_tracking_enabled = match root_config {
        Some(c) => c
            .parse::<RolloutPercentage>(BsmrconfigKeyRef {
                section: "bsmr",
                property: "invalidation_tracking_enabled",
            })?
            .is_some_and(|v| v.roll()),
        None => false,
    };
    dice.set_invalidation_tracking_config(invalidation_tracking_enabled);

    // Empty handle; a command enables the tracker lazily if it needs one.
    dice.set_detailed_aggregated_metrics_handle(DetailedAggregatedMetricsHandle::new());

    // Opt-in pagable storage. When `BSMR_DICE_DB_PATH` is set, configures a
    // `DiceStorage` backend, configured by `PAGABLE_STORAGE_BACKEND` so `Dice::page_out()`
    // (e.g. via `bsmr debug hydration page-out`) can serialize node values to disk.
    if let Ok(path) = std::env::var("BSMR_DICE_DB_PATH") {
        let storage = DiceStorage::open(std::path::Path::new(&path)).map_err(|e| {
            bsmr_error::conversion::from_any_with_tag(e, bsmr_error::ErrorTag::Environment)
        })?;
        dice.set_pagable_storage(storage);
    }

    let dice = dice.build(detect_cycles);
    let mut dice_ctx = dice.updater();
    dice_ctx.set_none_cell_resolver()?;
    dice_ctx.set_none_legacy_config_external_data()?;
    dice_ctx.commit().await;

    Ok(dice)
}
