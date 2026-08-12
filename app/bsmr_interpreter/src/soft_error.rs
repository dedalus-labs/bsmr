//===----------------------------------------------------------------------===//
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

use bsmr_core::soft_error;
use starlark::eval::SoftErrorHandler;
pub struct BsmrStarlarkSoftErrorHandler;

/// When starlark deprecates something, we propagate it to our `soft_error!` handler.
impl SoftErrorHandler for BsmrStarlarkSoftErrorHandler {
    fn soft_error(&self, category: &str, error: starlark::Error) -> Result<(), starlark::Error> {
        let error = bsmr_error::Error::from(error);
        soft_error!(&format!("starlark_rust_{category}"), error, deprecation: true, quiet: true, error_on_oss: true)?;
        Ok(())
    }
}
