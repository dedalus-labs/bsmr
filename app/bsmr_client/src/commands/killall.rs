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

use bsmr_client_ctx::client_ctx::BsmrSubcommand;
use bsmr_client_ctx::client_ctx::ClientCommandContext;
use bsmr_client_ctx::common::BsmrArgMatches;
use bsmr_client_ctx::common::CommonEventLogOptions;
use bsmr_client_ctx::events_ctx::EventsCtx;
use bsmr_client_ctx::exit_result::ExitResult;
use bsmr_wrapper_common::is_bsmr::WhoIsAsking;

#[derive(Debug, clap::Parser)]
#[clap(about = "Kill all bsmr processes on the machine")]
pub struct KillallCommand {
    #[clap(flatten)]
    pub(crate) event_log_opts: CommonEventLogOptions,
}

impl BsmrSubcommand for KillallCommand {
    const COMMAND_NAME: &'static str = "killall";

    async fn exec_impl(
        self,
        _matches: BsmrArgMatches<'_>,
        _ctx: ClientCommandContext<'_>,
        _events_ctx: &mut EventsCtx,
    ) -> ExitResult {
        bsmr_wrapper_common::killall(WhoIsAsking::Bessemer, |s| {
            let _ignored = bsmr_client_ctx::eprintln!("{}", s);
        })
        .then_some(())
        .ok_or(bsmr_error::bsmr_error!(
            bsmr_error::ErrorTag::KillAll,
            "Killall command failed"
        ))
        .into()
    }

    fn event_log_opts(&self) -> &CommonEventLogOptions {
        &self.event_log_opts
    }
}
