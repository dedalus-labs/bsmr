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

use bsmr_cli_proto::SetLogFilterRequest;
use bsmr_client_ctx::client_ctx::ClientCommandContext;
use bsmr_client_ctx::common::BsmrArgMatches;
use bsmr_client_ctx::daemon::client::connect::BsmrdConnectOptions;
use bsmr_client_ctx::daemon::client::connect::connect_bsmrd;
use bsmr_client_ctx::events_ctx::EventsCtx;
use bsmr_client_ctx::exit_result::ExitResult;
use bsmr_client_ctx::subscribers::stdout_stderr_forwarder::StdoutStderrForwarder;

/// Change the log filter that's currently applied by the Bessemer daemon.
#[derive(Debug, clap::Parser)]
#[clap()]
pub struct SetLogFilterCommand {
    /// The log filter to apply.
    #[clap()]
    log_filter: String,

    /// Whether not to apply it to the daemon.
    #[clap(long)]
    no_daemon: bool,

    /// Whether not to apply it to the forkserver.
    #[clap(long)]
    no_forkserver: bool,
}

impl SetLogFilterCommand {
    pub fn exec(self, _matches: BsmrArgMatches<'_>, ctx: ClientCommandContext<'_>) -> ExitResult {
        ctx.with_runtime(|ctx| async move {
            let mut events_ctx = EventsCtx::new(None, vec![Box::new(StdoutStderrForwarder)]);

            let mut bsmrd = connect_bsmrd(
                BsmrdConnectOptions::ExistingOnly,
                &mut events_ctx,
                ctx.paths()?,
            )
            .await?;

            bsmrd
                .with_flushing()
                .set_log_filter(
                    &mut events_ctx,
                    SetLogFilterRequest {
                        log_filter: self.log_filter,
                        daemon: !self.no_daemon,
                        forkserver: !self.no_forkserver,
                    },
                )
                .await?;

            ExitResult::success()
        })
    }
}
