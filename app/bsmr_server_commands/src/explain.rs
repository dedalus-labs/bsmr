/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use bsmr_cli_proto::new_generic::ExplainRequest;
use bsmr_cli_proto::new_generic::ExplainResponse;
use bsmr_server_ctx::ctx::ServerCommandContextTrait;
use bsmr_server_ctx::partial_result_dispatcher::NoPartialResult;
use bsmr_server_ctx::partial_result_dispatcher::PartialResultDispatcher;

pub(crate) async fn explain_command(
    _ctx: &dyn ServerCommandContextTrait,
    _partial_result_dispatcher: PartialResultDispatcher<NoPartialResult>,
    _req: ExplainRequest,
) -> bsmr_error::Result<ExplainResponse> {
    Err(bsmr_error::bsmr_error!(
        bsmr_error::ErrorTag::Unimplemented,
        "explain is not supported"
    ))
}
