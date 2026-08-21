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
use bsmr_client_ctx::stdio;
use tokio_stream::StreamExt;

/// Outputs the log in JSON format from selected invocation.
#[derive(Debug, clap::Parser)]
pub struct ShowLogCommand {
    #[clap(flatten)]
    event_log: EventLogOptions,
}

impl BsmrSubcommand for ShowLogCommand {
    const COMMAND_NAME: &'static str = "log-show";

    async fn exec_impl(
        self,
        _matches: BsmrArgMatches<'_>,
        ctx: ClientCommandContext<'_>,
        _events_ctx: &mut EventsCtx,
    ) -> ExitResult {
        let Self { event_log } = self;
        let log_path = event_log.get(&ctx).await?;

        let (invocation, mut events) = log_path.unpack_stream().await?;

        let mut buf = Vec::new();

        serde_json::to_writer(&mut buf, &invocation.to_proto())?;
        stdio::print_bytes(&buf)?;
        stdio::print_bytes(b"\n")?;

        while let Some(event) = events.try_next().await? {
            buf.clear();
            serde_json::to_writer(&mut buf, &event)?;
            stdio::print_bytes(&buf)?;
            stdio::print_bytes(b"\n")?;
        }

        ExitResult::success()
    }
}
