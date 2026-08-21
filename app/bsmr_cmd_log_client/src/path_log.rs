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
use bsmr_client_ctx::event_log_options::EventLogOptions;
use bsmr_client_ctx::events_ctx::EventsCtx;
use bsmr_client_ctx::exit_result::ExitResult;
use bsmr_error::BsmrErrorContext;
use bsmr_event_log::file_names::retrieve_all_logs;

/// Output the path to the selected log.
#[derive(Debug, clap::Parser)]
pub struct PathLogCommand {
    /// Find the log from the Nth most recent command (`--recent 0` is the most recent).
    #[clap(flatten)]
    event_log_options: EventLogOptions,

    /// List all the logs.
    #[clap(long, group = "event_log")]
    all: bool,
}

impl BsmrSubcommand for PathLogCommand {
    const COMMAND_NAME: &'static str = "log-path";

    async fn exec_impl(
        self,
        _matches: BsmrArgMatches<'_>,
        ctx: ClientCommandContext<'_>,
        _events_ctx: &mut EventsCtx,
    ) -> ExitResult {
        let Self {
            event_log_options,
            all,
        } = self;

        let paths = if all {
            retrieve_all_logs(
                ctx.paths()
                    .bsmr_error_context("Error identifying log dir")?,
            )?
        } else {
            vec![event_log_options.get(&ctx).await?]
        };
        for path in paths {
            bsmr_client_ctx::println!("{}", path.path().display())?;
        }
        ExitResult::success()
    }
}
