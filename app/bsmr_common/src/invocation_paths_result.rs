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

use crate::invocation_paths::InvocationPaths;

#[derive(Clone)]
pub enum InvocationPathsResult {
    OtherError(bsmr_error::Error),
    Paths(InvocationPaths),
    OutsideOfRepo(bsmr_error::Error), // this error ignored for creating invocation record for log commands
}

impl InvocationPathsResult {
    pub fn get_result(self) -> bsmr_error::Result<InvocationPaths> {
        match self {
            InvocationPathsResult::OtherError(e) => Err(e),
            InvocationPathsResult::Paths(paths) => Ok(paths),
            InvocationPathsResult::OutsideOfRepo(e) => Err(e),
        }
    }
}
