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

use bsmr_error::BsmrErrorContext;
use bsmr_util::tokio_runtime::new_tokio_runtime;

/// Tokio runtime used by the client commands.
pub fn client_tokio_runtime() -> bsmr_error::Result<tokio::runtime::Runtime> {
    // Do not use current thread because current thread may have too low thread size.
    new_tokio_runtime("bsmr-cli")
        // Tokio creates this number of threads,
        // and creating too many threads for short commands is expensive.
        .worker_threads(1)
        .enable_all()
        .build()
        .bsmr_error_context("Building tokio runtime")
}
