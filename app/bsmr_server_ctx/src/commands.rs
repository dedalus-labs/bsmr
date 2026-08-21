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

use bsmr_core::provider::label::ConfiguredProvidersLabel;
use bsmr_events::dispatch::EventDispatcher;
use bsmr_hash::StdBsmrHashSet;

/// Common code executed in the end of command to produce `CommandEnd`.
pub fn command_end<R, D>(result: &bsmr_error::Result<R>, data: D) -> bsmr_data::CommandEnd
where
    D: Into<bsmr_data::command_end::Data>,
{
    command_end_ext(result, data.into(), |_| None)
}

pub fn command_end_ext<R, D, F>(
    result: &bsmr_error::Result<R>,
    data: D,
    build_result: F,
) -> bsmr_data::CommandEnd
where
    F: FnOnce(&R) -> Option<bsmr_data::BuildResult>,
    D: Into<bsmr_data::command_end::Data>,
{
    bsmr_data::CommandEnd {
        data: Some(data.into()),
        build_result: result.as_ref().ok().and_then(build_result),
        ..Default::default()
    }
}

/// Common code to send TargetCfg event after command execution.
pub fn send_target_cfg_event(
    event_dispatcher: &EventDispatcher,
    conf_labels: impl IntoIterator<Item = &ConfiguredProvidersLabel>,
    target_cfg: &Option<bsmr_cli_proto::TargetCfg>,
) {
    let mut target_platforms = StdBsmrHashSet::default();
    for conf in conf_labels {
        // cfg can be unbound
        if let Ok(label) = conf.cfg().label() {
            if !target_platforms.contains(label) {
                target_platforms.insert(label.to_owned());
            }
        }
    }

    let cli_modifiers = target_cfg
        .as_ref()
        .map(|cfg| cfg.cli_modifiers.clone())
        .unwrap_or_default();

    event_dispatcher.instant_event(bsmr_data::TargetCfg {
        target_platforms: target_platforms.into_iter().collect(),
        cli_modifiers,
    });
}
