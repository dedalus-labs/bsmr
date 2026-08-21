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

use std::time::Duration;

use bsmr_client_ctx::client_ctx::BsmrSubcommand;
use bsmr_client_ctx::client_ctx::ClientCommandContext;
use bsmr_client_ctx::common::BsmrArgMatches;
use bsmr_client_ctx::common::CommonEventLogOptions;
use bsmr_client_ctx::daemon::client::BsmrdLifecycleLock;
use bsmr_client_ctx::events_ctx::EventsCtx;
use bsmr_client_ctx::exit_result::ExitResult;
use bsmr_client_ctx::startup_deadline::StartupDeadline;

/// Kill the Bessemer daemon.
///
/// Note there's also `bsmr killall` and `bsmr clean`.
///
/// `bsmr killall` kills all the bsmr processes on the machine.
///
/// `bsmr clean` kills the bsmr daemon and also deletes the bsmr state files.
#[derive(Debug, clap::Parser)]
pub struct KillCommand {
    #[clap(flatten)]
    pub(crate) event_log_opts: CommonEventLogOptions,
}

impl BsmrSubcommand for KillCommand {
    const COMMAND_NAME: &'static str = "kill";

    async fn exec_impl(
        self,
        _matches: BsmrArgMatches<'_>,
        ctx: ClientCommandContext<'_>,
        _events_ctx: &mut EventsCtx,
    ) -> ExitResult {
        let daemon_dir = ctx.paths()?.daemon_dir()?;

        let lifecycle_lock = BsmrdLifecycleLock::lock_with_timeout(
            daemon_dir.clone(),
            StartupDeadline::duration_from_now(Duration::from_secs(10))?,
        )
        .await?;

        bsmr_client_ctx::daemon::client::kill::kill_command_impl(
            &lifecycle_lock,
            "`bsmr kill` was invoked",
        )
        .await
        .into()
    }

    fn event_log_opts(&self) -> &CommonEventLogOptions {
        &self.event_log_opts
    }
}
