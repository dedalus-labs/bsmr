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
use bsmr_error::BuckErrorContext;
use bsmr_error::internal_error;
use bsmr_execute::materialize::materializer::CleanStaleArtifactsArgs;
use bsmr_server_ctx::ctx::ServerCommandContextTrait;
use bsmr_server_ctx::partial_result_dispatcher::NoPartialResult;
use bsmr_server_ctx::partial_result_dispatcher::PartialResultDispatcher;
use bsmr_server_ctx::template::ServerCommandTemplate;
use bsmr_server_ctx::template::run_server_command;
use chrono::TimeZone;
use chrono::Utc;
use dice::DiceTransaction;

use crate::ctx::ServerCommandContext;

pub(crate) async fn clean_stale_command(
    ctx: &ServerCommandContext<'_>,
    partial_result_dispatcher: PartialResultDispatcher<NoPartialResult>,
    req: bsmr_cli_proto::CleanStaleRequest,
) -> bsmr_error::Result<bsmr_cli_proto::CleanStaleResponse> {
    run_server_command(
        CleanStaleServerCommand { req },
        ctx,
        partial_result_dispatcher,
    )
    .await
}

struct CleanStaleServerCommand {
    req: bsmr_cli_proto::CleanStaleRequest,
}

#[async_trait]
impl ServerCommandTemplate for CleanStaleServerCommand {
    type StartEvent = bsmr_data::CleanCommandStart;
    type EndEvent = bsmr_data::CleanCommandEnd;
    type Response = bsmr_cli_proto::CleanStaleResponse;
    type PartialResult = NoPartialResult;

    async fn command(
        &self,
        server_ctx: &dyn ServerCommandContextTrait,
        _partial_result_dispatcher: PartialResultDispatcher<Self::PartialResult>,
        _ctx: DiceTransaction,
    ) -> bsmr_error::Result<Self::Response> {
        server_ctx
            .cancellation_context()
            .critical_section(|| async move {
                let deferred_materializer = server_ctx.materializer();

                let extension = deferred_materializer
                    .as_deferred_materializer_extension()
                    .ok_or_else(|| internal_error!("Deferred materializer is not in use"))?;

                let keep_since_time = Utc
                    .timestamp_opt(self.req.keep_since_time, 0)
                    .single()
                    .ok_or_else(|| internal_error!("Invalid timestamp"))?;

                let adaptive_min_ttl = self
                    .req
                    .adaptive_min_ttl_seconds
                    .map(|s| std::time::Duration::from_secs(s.max(0) as u64));
                extension
                    .clean_stale_artifacts(CleanStaleArtifactsArgs {
                        keep_since_time,
                        dry_run: self.req.dry_run,
                        tracked_only: self.req.tracked_only,
                        adaptive_low_disk_threshold: self.req.adaptive_low_disk_threshold,
                        adaptive_min_ttl,
                    })
                    .await
                    .buck_error_context("Failed to clean stale artifacts.")
            })
            .await
    }

    fn end_event(&self, response: &bsmr_error::Result<Self::Response>) -> Self::EndEvent {
        let clean_stale_stats = if let Ok(res) = response {
            res.stats
        } else {
            None
        };
        bsmr_data::CleanCommandEnd { clean_stale_stats }
    }
}
