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

//! Starlark debugging.

pub mod configured_graph_size;

use async_trait::async_trait;
use bsmr_client_ctx::common::CommonCommandOptions;

use crate::AuditSubcommand;
use crate::perf::configured_graph_size::ConfiguredGraphSizeCommand;

#[derive(Debug, clap::Subcommand, serde::Serialize, serde::Deserialize)]
#[clap(name = "perf", about = "Commands for checking bsmr performance")]
pub enum AuditPerfCommand {
    ConfiguredGraphSize(ConfiguredGraphSizeCommand),
}

#[async_trait]
impl AuditSubcommand for AuditPerfCommand {
    fn common_opts(&self) -> &CommonCommandOptions {
        match self {
            AuditPerfCommand::ConfiguredGraphSize(cmd) => &cmd.common_opts,
        }
    }
}
