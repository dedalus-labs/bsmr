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

//! Starlark debugging.

mod module;
mod package_deps;

use async_trait::async_trait;
use bsmr_cli_proto::ClientContext;
use bsmr_cmd_audit_client::starlark::StarlarkCommand;
use bsmr_server_ctx::ctx::ServerCommandContextTrait;
use bsmr_server_ctx::partial_result_dispatcher::PartialResultDispatcher;

use crate::ServerAuditSubcommand;

#[async_trait]
impl ServerAuditSubcommand for StarlarkCommand {
    async fn server_execute(
        &self,
        server_ctx: &dyn ServerCommandContextTrait,
        stdout: PartialResultDispatcher<bsmr_cli_proto::StdoutBytes>,
        client_ctx: ClientContext,
    ) -> bsmr_error::Result<()> {
        match self {
            StarlarkCommand::Module(cmd) => {
                module::server_execute(cmd, server_ctx, stdout, client_ctx).await
            }
            StarlarkCommand::PackageDeps(cmd) => {
                package_deps::server_execute(cmd, server_ctx, stdout, client_ctx).await
            }
        }
    }
}
