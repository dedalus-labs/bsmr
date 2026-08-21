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

use starlark::environment::GlobalsBuilder;
use starlark::eval::Evaluator;
use starlark::starlark_module;
use starlark::values::none::NoneType;

use crate::interpreter::module_internals::ModuleInternals;

#[derive(bsmr_error::Error, Debug)]
#[error("Fail: {0}")]
#[bsmr(tag = Tier0)]
struct BsmrFail(String);

/// Registers functions that are only available in the `__internal__` global and not meant to be
/// stable.
#[starlark_module]
pub(crate) fn register_internals(builder: &mut GlobalsBuilder) {
    /// `fail()` but implemented using a bsmr error type instead of starlark's, for testing
    /// purposes.
    fn bsmr_fail<'v>(msg: &str, _eval: &mut Evaluator<'v, '_, '_>) -> starlark::Result<NoneType> {
        Err(bsmr_error::Error::from(BsmrFail(msg.to_owned())).into())
    }

    /// Returns a list of direct subpackage relative paths of current package.
    fn sub_packages<'v>(eval: &mut Evaluator<'v, '_, '_>) -> starlark::Result<Vec<String>> {
        let extra = ModuleInternals::from_context(eval, "sub_packages")?;
        Ok(extra
            .sub_packages()
            .map(|p| p.as_str().to_owned())
            .collect())
    }
}
