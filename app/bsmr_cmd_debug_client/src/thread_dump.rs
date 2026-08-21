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
use bsmr_client_ctx::daemon::client::connect::BsmrdProcessInfo;
use bsmr_client_ctx::exit_result::ExitResult;
use bsmr_client_ctx::thread_dump::thread_dump_command;
use bsmr_error::BsmrErrorContext;
use bsmr_error::ErrorTag;
use bsmr_error::bsmr_error;

/// Prints a thread dump of the currently running bsmr daemon to stdout
#[derive(Debug, clap::Parser)]
pub struct ThreadDumpCommand {}

impl ThreadDumpCommand {
    pub fn exec(self, _matches: BsmrArgMatches<'_>, ctx: ClientCommandContext<'_>) -> ExitResult {
        let paths = ctx.paths()?;
        let daemon_dir = paths.daemon_dir()?;
        let Ok(info) = BsmrdProcessInfo::load(&daemon_dir) else {
            return bsmr_error!(ErrorTag::Input, "No running bsmr daemon").into();
        };

        ctx.with_runtime(|_| async move {
            let status = thread_dump_command(&info)?
                .spawn()
                .bsmr_error_context("Could not run LLDB to grab a thread-dump")?
                .wait()
                .await?;
            if status.success() {
                bsmr_error::Ok(ExitResult::success())
            } else {
                // We don't capture stderr, so lldb should have printed an error
                bsmr_error::Ok(ExitResult::err(bsmr_error!(
                    ErrorTag::Tier0,
                    "Thread dump command failed"
                )))
            }
        })?
    }
}
