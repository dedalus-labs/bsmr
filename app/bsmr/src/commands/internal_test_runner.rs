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

use bsmr_client_ctx::client_ctx::ClientCommandContext;
use bsmr_client_ctx::common::BsmrArgMatches;
use bsmr_client_ctx::events_ctx::EventsCtx;
use bsmr_client_ctx::exit_result::ExitResult;
use bsmr_error::ErrorTag;
use clap::Parser;
use tokio::runtime::Runtime;

#[derive(Debug, Parser)]
#[clap(about = "run the internal test runner")]
pub(crate) struct InternalTestRunnerCommand {
    #[cfg(unix)]
    #[clap(flatten)]
    unix_runner: bsmr_test_runner::unix::BsmrTestRunnerUnix,

    #[cfg(not(unix))]
    #[clap(flatten)]
    tcp_runner: bsmr_test_runner::tcp::BsmrTestRunnerTcp,
}

impl InternalTestRunnerCommand {
    pub(crate) fn exec(
        self,
        _matches: BsmrArgMatches<'_>,
        _ctx: ClientCommandContext<'_>,
        events_ctx: &mut EventsCtx,
    ) -> ExitResult {
        events_ctx.log_invocation_record = false;

        // Internal test runner should only be used in the open source version of Bessemer.
        if bsmr_core::is_open_source()
            || std::env::var("BSMR_ALLOW_INTERNAL_TEST_RUNNER_DO_NOT_USE").is_ok()
        {
            let runtime = Runtime::new().expect("Failed to create Tokio runtime");
            runtime
                .block_on(async move {
                    #[cfg(unix)]
                    {
                        self.unix_runner.run().await
                    }
                    #[cfg(not(unix))]
                    {
                        self.tcp_runner.run().await
                    }
                })
                .into()
        } else {
            bsmr_error::bsmr_error!(
                ErrorTag::Input,
                "Cannot use internal test runner. Config value must be provided for test.v2_test_executor."
            ).into()
        }
    }
}
