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

use bsmr_client_ctx::common::target_cfg::TargetCfgWithUniverseOptions;
use bsmr_server_ctx::ctx::ServerCommandContextTrait;
use bsmr_server_ctx::target_resolution_config::TargetResolutionConfig;
use dice::DiceComputations;

pub(crate) async fn audit_command_target_resolution_config(
    ctx: &mut DiceComputations<'_>,
    target_cfg: &TargetCfgWithUniverseOptions,
    server_ctx: &dyn ServerCommandContextTrait,
) -> bsmr_error::Result<TargetResolutionConfig> {
    TargetResolutionConfig::from_args(
        ctx,
        &target_cfg.target_cfg.target_cfg(),
        server_ctx,
        &target_cfg.target_universe,
    )
    .await
}
