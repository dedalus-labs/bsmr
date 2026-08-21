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
use bsmr_cli_proto::GenericRequest;
use bsmr_client_ctx::client_ctx::ClientCommandContext;
use bsmr_client_ctx::common::BsmrArgMatches;
use bsmr_client_ctx::common::CommonBuildConfigurationOptions;
use bsmr_client_ctx::common::CommonEventLogOptions;
use bsmr_client_ctx::common::CommonStarlarkOptions;
use bsmr_client_ctx::common::ui::CommonConsoleOptions;
use bsmr_client_ctx::daemon::client::BsmrdClientConnector;
use bsmr_client_ctx::daemon::client::StdoutPartialResultHandler;
use bsmr_client_ctx::events_ctx::EventsCtx;
use bsmr_client_ctx::exit_result::ExitResult;
use bsmr_client_ctx::streaming::StreamingCommand;
use bsmr_common::argv::Argv;
use bsmr_common::argv::SanitizedArgv;

use crate::debug::StarlarkDebugAttachCommand;
use crate::lint::StarlarkLintCommand;
use crate::typecheck::StarlarkTypecheckCommand;

mod debug;
pub mod lint;
pub mod typecheck;

#[derive(Debug, clap::Subcommand)]
#[clap(name = "starlark", about = "Run Starlark operations")]
pub enum StarlarkCommand {
    #[clap(flatten)]
    Opaque(StarlarkSubcommand),
    DebugAttach(StarlarkDebugAttachCommand),
}

// Used for subcommands that follow `bsmr audit`'s "opaque" pattern where the command object is serialized
// to the daemon and deserialized there and has a `server_execute()` on the Command object itself (as opposed
// to using structured endpoints in the daemon protocol).
#[derive(Debug, clap::Subcommand, serde::Serialize, serde::Deserialize)]
pub enum StarlarkSubcommand {
    Lint(StarlarkLintCommand),
    Typecheck(StarlarkTypecheckCommand),
}

impl StarlarkSubcommand {
    fn as_client_subcommand(&self) -> &dyn StarlarkClientSubcommand {
        match self {
            StarlarkSubcommand::Lint(cmd) => cmd,
            StarlarkSubcommand::Typecheck(cmd) => cmd,
        }
    }
}

trait StarlarkClientSubcommand {
    fn common_opts(&self) -> &StarlarkCommandCommonOptions;
}

#[derive(Debug, clap::Parser, serde::Serialize, serde::Deserialize, Default)]
pub struct StarlarkCommandCommonOptions {
    #[clap(flatten)]
    config_opts: CommonBuildConfigurationOptions,

    #[clap(flatten)]
    starlark_opts: CommonStarlarkOptions,

    #[clap(flatten)]
    console_opts: CommonConsoleOptions,

    #[clap(flatten)]
    event_log_opts: CommonEventLogOptions,
}

#[async_trait(?Send)]
impl StreamingCommand for StarlarkSubcommand {
    const COMMAND_NAME: &'static str = "starlark";

    /// Starlark subcommands are all implemented as a generic request to the bsmrd server that will deserialize the command object.
    async fn exec_impl(
        self,
        bsmrd: &mut BsmrdClientConnector,
        matches: BsmrArgMatches<'_>,
        ctx: &mut ClientCommandContext<'_>,
        events_ctx: &mut EventsCtx,
    ) -> ExitResult {
        let serialized = serde_json::to_string(&self)?;

        let context = ctx.client_context(matches, &self)?;

        bsmrd
            .with_flushing()
            .starlark(
                GenericRequest {
                    context: Some(context),
                    serialized_opts: serialized,
                },
                events_ctx,
                ctx.console_interaction_stream(self.console_opts()),
                &mut StdoutPartialResultHandler,
            )
            .await??;
        ExitResult::success()
    }

    fn console_opts(&self) -> &CommonConsoleOptions {
        &self.as_client_subcommand().common_opts().console_opts
    }

    fn event_log_opts(&self) -> &CommonEventLogOptions {
        &self.as_client_subcommand().common_opts().event_log_opts
    }

    fn build_config_opts(&self) -> &CommonBuildConfigurationOptions {
        &self.as_client_subcommand().common_opts().config_opts
    }

    fn starlark_opts(&self) -> &CommonStarlarkOptions {
        &self.as_client_subcommand().common_opts().starlark_opts
    }
}

impl StarlarkCommand {
    pub fn exec(
        self,
        matches: BsmrArgMatches<'_>,
        ctx: ClientCommandContext<'_>,
        events_ctx: &mut EventsCtx,
    ) -> ExitResult {
        let matches = matches.unwrap_subcommand();
        match self {
            StarlarkCommand::Opaque(cmd) => ctx.exec(cmd, matches, events_ctx),
            StarlarkCommand::DebugAttach(cmd) => ctx.exec(cmd, matches, events_ctx),
        }
    }

    pub fn sanitize_argv(&self, argv: Argv) -> SanitizedArgv {
        argv.no_need_to_sanitize()
    }

    pub fn command_name(&self) -> &'static str {
        match self {
            StarlarkCommand::Opaque(_) => "starlark",
            StarlarkCommand::DebugAttach(_) => "starlark-debug-attach",
        }
    }
}
