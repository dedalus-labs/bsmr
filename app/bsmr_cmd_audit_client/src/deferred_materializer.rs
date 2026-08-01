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

use async_trait::async_trait;
use bsmr_client_ctx::common::CommonCommandOptions;
use bsmr_client_ctx::common::target_cfg::TargetCfgUnusedOptions;

use crate::AuditSubcommand;

#[derive(Debug, clap::Parser, serde::Serialize, serde::Deserialize)]
#[clap(
    name = "deferred-materializer",
    about = "Access and interact with the deferred materializer"
)]
pub struct DeferredMaterializerCommand {
    #[clap(subcommand)]
    pub subcommand: DeferredMaterializerSubcommand,

    /// Command doesn't need these flags, but they are used in mode files, so we need to keep them.
    #[clap(flatten)]
    pub _target_cfg: TargetCfgUnusedOptions,

    #[clap(flatten)]
    pub common_opts: CommonCommandOptions,
}

#[derive(Debug, clap::Subcommand, serde::Serialize, serde::Deserialize)]
pub enum DeferredMaterializerSubcommand {
    List,
    ListSubscriptions,
    Fsck,
    Refresh {
        /// Minimum TTL to require for actions.
        #[clap()]
        min_ttl: i64,
    },
    /// Get the log for TTL refreshes.
    GetRefreshLog,
    TestIter {
        #[clap(long, default_value = "1")]
        count: usize,
    },
    FlushAccessTimes,
}

#[async_trait]
impl AuditSubcommand for DeferredMaterializerCommand {
    fn common_opts(&self) -> &CommonCommandOptions {
        &self.common_opts
    }
}
