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
use bsmr_client_ctx::common::BuckArgMatches;
use bsmr_client_ctx::exit_result::ExitResult;

/// Print bsmr daemon directory (`~/.buckd/xxx`).
#[derive(Debug, clap::Parser)]
pub struct DaemonDirCommand {}

impl DaemonDirCommand {
    pub fn exec(self, _matches: BuckArgMatches<'_>, ctx: ClientCommandContext<'_>) -> ExitResult {
        bsmr_client_ctx::println!("{}", ctx.paths()?.daemon_dir()?.path.display())?;
        ExitResult::success()
    }
}
