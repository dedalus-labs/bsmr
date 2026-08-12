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

use bsmr_node::attrs::attr_type::source::SourceAttrType;
use bsmr_node::attrs::coerced_attr::CoercedAttr;
use bsmr_node::attrs::coercion_context::AttrCoercionContext;
use bsmr_node::attrs::configurable::AttrIsConfigurable;
use starlark::typing::Ty;
use starlark::values::Value;

use crate::attrs::coerce::AttrTypeCoerce;
use crate::attrs::coerce::attr_type::ty_maybe_select::TyMaybeSelect;

#[derive(Debug, bsmr_error::Error)]
#[bsmr(input)]
enum SourceLabelCoercionError {
    #[error(
        "Couldn't coerce `{0}` as a source.\n  Error when treated as a target: {1:#}\n  Error when treated as a path: {2:#}"
    )]
    CoercionFailed(String, bsmr_error::Error, bsmr_error::Error),
}

/// Try cleaning up irrelevant details users often type
fn cleanup_path(value: &str) -> &str {
    let value = value.strip_prefix("./").unwrap_or(value);
    let value = value.strip_suffix("/").unwrap_or(value);
    if value == "." { "" } else { value }
}

impl AttrTypeCoerce for SourceAttrType {
    fn coerce_item(
        &self,
        _configurable: AttrIsConfigurable,
        ctx: &dyn AttrCoercionContext,
        value: Value,
    ) -> bsmr_error::Result<CoercedAttr> {
        let source_label = value.unpack_str_err()?;

        let label_err = if source_label.contains(':') {
            match ctx.coerce_providers_label(source_label) {
                Ok(l) => return Ok(CoercedAttr::SourceLabel(l)),
                Err(e) => Some(e),
            }
        } else {
            // As an optimization, we skip trying to parse as a label in this case
            None
        };

        let path_err = match ctx.coerce_path(cleanup_path(source_label), self.allow_directory) {
            Ok(path) => return Ok(CoercedAttr::SourceFile(path)),
            Err(path_err) => path_err,
        };

        if let Some(label_err) = label_err {
            Err(
                SourceLabelCoercionError::CoercionFailed(value.to_str(), label_err, path_err)
                    .into(),
            )
        } else {
            Err(path_err.context(format!("Coercing `{}` as a source", value.to_str())))
        }
    }

    fn starlark_type(&self) -> TyMaybeSelect {
        TyMaybeSelect::Basic(Ty::string())
    }
}
