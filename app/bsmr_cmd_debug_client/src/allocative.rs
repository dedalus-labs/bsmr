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
use bsmr_cli_proto::AllocativeRequest;
use bsmr_client_ctx::client_ctx::ClientCommandContext;
use bsmr_client_ctx::common::BsmrArgMatches;
use bsmr_client_ctx::common::CommonBuildConfigurationOptions;
use bsmr_client_ctx::common::CommonEventLogOptions;
use bsmr_client_ctx::common::CommonStarlarkOptions;
use bsmr_client_ctx::common::ui::CommonConsoleOptions;
use bsmr_client_ctx::daemon::client::BsmrdClientConnector;
use bsmr_client_ctx::daemon::client::NoPartialResultHandler;
use bsmr_client_ctx::events_ctx::EventsCtx;
use bsmr_client_ctx::exit_result::ExitResult;
use bsmr_client_ctx::path_arg::PathArg;
use bsmr_client_ctx::streaming::StreamingCommand;

#[derive(Debug, clap::Parser)]
pub struct AllocativeCommand {
    /// Output directory path for profile data.
    ///
    /// Directory will be created if it does not exist.
    #[clap(
        long,
        short = 'o',
        value_name = "PATH",
        default_value = "allocative-out"
    )]
    output: PathArg,
}

#[async_trait(?Send)]
impl StreamingCommand for AllocativeCommand {
    const COMMAND_NAME: &'static str = "allocative";

    fn existing_only() -> bool {
        true
    }

    async fn exec_impl(
        self,
        bsmrd: &mut BsmrdClientConnector,
        _matches: BsmrArgMatches<'_>,
        ctx: &mut ClientCommandContext<'_>,
        events_ctx: &mut EventsCtx,
    ) -> ExitResult {
        let context = ctx.empty_client_context("debug-allocative")?;
        bsmrd
            .with_flushing()
            .allocative(
                AllocativeRequest {
                    context: Some(context),
                    output_path: self.output.resolve(&ctx.working_dir).into_string()?,
                },
                events_ctx,
                ctx.console_interaction_stream(self.console_opts()),
                &mut NoPartialResultHandler,
            )
            .await??;
        ExitResult::success()
    }

    fn console_opts(&self) -> &CommonConsoleOptions {
        CommonConsoleOptions::default_ref()
    }

    fn event_log_opts(&self) -> &CommonEventLogOptions {
        CommonEventLogOptions::default_ref()
    }

    fn build_config_opts(&self) -> &CommonBuildConfigurationOptions {
        CommonBuildConfigurationOptions::default_ref()
    }

    fn starlark_opts(&self) -> &CommonStarlarkOptions {
        CommonStarlarkOptions::default_ref()
    }
}
