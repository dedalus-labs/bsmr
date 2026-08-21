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
use bsmr_cli_proto::ClientContext;
use bsmr_cmd_audit_client::output::parse::AuditParseCommand;
use bsmr_common::dice::cells::HasCellResolver;
use bsmr_server_ctx::ctx::ServerCommandContextTrait;
use bsmr_server_ctx::ctx::ServerCommandDiceContext;
use bsmr_server_ctx::partial_result_dispatcher::PartialResultDispatcher;

use super::output_path_parser::OutputPathParser;
use super::output_path_type_printer::OutputPathTypePrinter;
use crate::ServerAuditSubcommand;

#[async_trait]
impl ServerAuditSubcommand for AuditParseCommand {
    async fn server_execute(
        &self,
        server_ctx: &dyn ServerCommandContextTrait,
        mut stdout: PartialResultDispatcher<bsmr_cli_proto::StdoutBytes>,
        _client_ctx: ClientContext,
    ) -> bsmr_error::Result<()> {
        server_ctx
            .with_dice_ctx(|_server_ctx, mut dice_ctx| async move {
                let cell_resolver = dice_ctx.get_cell_resolver().await?;
                let output_parser = OutputPathParser::new(cell_resolver);
                let parsed_path = output_parser.parse(&self.output_path)?;

                let printer = OutputPathTypePrinter::new(self.json, &self.output_attribute)?;

                let stdout = stdout.as_writer();

                printer.print(&parsed_path, stdout)
            })
            .await
    }
}
