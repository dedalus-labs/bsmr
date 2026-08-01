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

use bsmr_common::legacy_configs::dice::HasInjectedLegacyConfigs;
use bsmr_core::fs::project::ProjectRoot;
use bsmr_hash::StdBuckHashSet;
use dice::DiceTransaction;

use crate::experiment_util::get_experiment_tags;

/// Get all TPX experiments from bsmrconfig
///
/// This function retrieves all experiments from bsmrconfig that start with "experiments.tpx_"
/// and returns them as a HashSet with the "experiments.tpx_" prefix removed.
pub async fn get_tpx_experiments(
    mut ctx: DiceTransaction,
    project_root: &ProjectRoot,
) -> bsmr_error::Result<StdBuckHashSet<String>> {
    // Get all experiments from bsmrconfig
    if !ctx.is_injected_external_bsmrconfig_data_key_set().await? {
        return Ok(StdBuckHashSet::default());
    }

    let external_configs = ctx.get_injected_external_bsmrconfig_data().await?;
    let current_external_and_local_configs = external_configs
        .get_bsmrconfig_components(project_root)
        .await;
    let experiment_tags = get_experiment_tags(&current_external_and_local_configs);
    // Filter experiments that start with "experiments.tpx_"
    let tpx_experiments = experiment_tags
        .into_iter()
        .filter(|tag| tag.starts_with("experiments.tpx_"))
        .map(|tag| tag.replace("experiments.tpx_", ""))
        .collect::<StdBuckHashSet<String>>();

    Ok(tpx_experiments)
}
