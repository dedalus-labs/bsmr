/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::sync::Arc;

use async_trait::async_trait;
use bsmr_cli_proto::HydrationSubcommand;
use bsmr_common::memory;
use bsmr_server_ctx::ctx::ServerCommandContextTrait;
use bsmr_server_ctx::partial_result_dispatcher::NoPartialResult;
use bsmr_server_ctx::partial_result_dispatcher::PartialResultDispatcher;
use bsmr_server_ctx::template::ServerCommandTemplate;
use bsmr_server_ctx::template::run_server_command;
use dice::Dice;
use dice::DiceTransaction;
use dice::PagableStatus;
use dupe::Dupe;

use crate::ctx::ServerCommandContext;

pub(crate) async fn hydration_command(
    ctx: &ServerCommandContext<'_>,
    partial_result_dispatcher: PartialResultDispatcher<NoPartialResult>,
    req: bsmr_cli_proto::HydrationRequest,
) -> bsmr_error::Result<bsmr_cli_proto::HydrationResponse> {
    let dice = ctx.base_context.daemon.dice_manager.unsafe_dice().dupe();
    let subcommand = HydrationSubcommand::try_from(req.subcommand)?;
    run_server_command(
        HydrationServerCommand { dice, subcommand },
        ctx,
        partial_result_dispatcher,
    )
    .await
}

struct HydrationServerCommand {
    dice: Arc<Dice>,
    subcommand: HydrationSubcommand,
}

#[async_trait]
impl ServerCommandTemplate for HydrationServerCommand {
    type StartEvent = bsmr_data::HydrationCommandStart;
    type EndEvent = bsmr_data::HydrationCommandEnd;
    type Response = bsmr_cli_proto::HydrationResponse;
    type PartialResult = NoPartialResult;

    fn exclusive_command_name(&self) -> Option<String> {
        match self.subcommand {
            HydrationSubcommand::PageOut | HydrationSubcommand::PageIn => {
                Some("hydration".to_owned())
            }
            HydrationSubcommand::Status => None,
        }
    }

    async fn command(
        &self,
        _server_ctx: &dyn ServerCommandContextTrait,
        _partial_result_dispatcher: PartialResultDispatcher<Self::PartialResult>,
        _ctx: DiceTransaction,
    ) -> bsmr_error::Result<Self::Response> {
        match self.subcommand {
            HydrationSubcommand::PageOut => {
                self.dice.page_out().await.map_err(|e| {
                    bsmr_error::conversion::from_any_with_tag(e, bsmr_error::ErrorTag::Environment)
                })?;
                // metrics() blocks on the core-state thread, draining its FIFO
                // queue, so the page-out evictions are processed before we purge.
                let _ = self.dice.metrics();
                memory::purge_jemalloc()?;
                Ok(bsmr_cli_proto::HydrationResponse::default())
            }
            HydrationSubcommand::PageIn => {
                self.dice.page_in().await.map_err(|e| {
                    bsmr_error::conversion::from_any_with_tag(e, bsmr_error::ErrorTag::Environment)
                })?;
                Ok(bsmr_cli_proto::HydrationResponse::default())
            }
            HydrationSubcommand::Status => {
                let status = self.dice.pagable_status().await;
                Ok(bsmr_cli_proto::HydrationResponse {
                    summary: Some(format_status_summary(&status)),
                })
            }
        }
    }
}

fn format_status_summary(status: &PagableStatus) -> String {
    // `total_nodes` counts vacant/in-progress nodes too; the rest is "other".
    // saturating_sub guards an underflow the struct invariant already rules out.
    let other = status
        .total_nodes
        .saturating_sub(status.resident_count)
        .saturating_sub(status.paged_out_count);
    let mut summary = format!(
        "DICE hydration: {} nodes ({} resident, {} paged out, {} other)\n",
        status.total_nodes, status.resident_count, status.paged_out_count, other,
    );
    if !status.by_type.is_empty() {
        summary.push('\n');
        summary.push_str(&format!(
            "{:>12}  {:>12}  {}\n",
            "resident", "paged-out", "key type"
        ));
        for t in &status.by_type {
            summary.push_str(&format!(
                "{:>12}  {:>12}  {}\n",
                t.resident, t.paged_out, t.key_type
            ));
        }
    }
    summary
}
