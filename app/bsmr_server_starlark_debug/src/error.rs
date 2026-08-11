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

/// Errors from buck's starlark debugger
#[derive(Debug, bsmr_error::Error)]
#[bsmr(tag = Tier0)]
pub(crate) enum StarlarkDebuggerError {
    #[error("starlark debugger has not yet implemented this functionality")]
    Unimplemented,
    #[error("another debug-attach process is already attached")]
    DebuggerAlreadyAttached,
}

/// Internal errors from buck's starlark debugger
#[derive(Debug, bsmr_error::Error)]
#[bsmr(tag = Tier0)]
pub(crate) enum StarlarkDebuggerInternalError {
    #[error("Internal error: debbugger server shutdown unexpectedly")]
    UnexpectedDebuggerShutdown,
    #[error("Internal error: debug controller already initialized with Evaluator")]
    EvalWrapperAlreadyUsed,
}
