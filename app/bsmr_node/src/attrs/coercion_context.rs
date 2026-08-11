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

use bsmr_core::pattern::pattern::ParsedPattern;
use bsmr_core::pattern::pattern_type::TargetPatternExtra;
use bsmr_core::provider::label::NonDefaultProvidersName;
use bsmr_core::provider::label::ProvidersLabel;
use bsmr_core::provider::label::ProvidersName;
use bsmr_core::target::label::label::TargetLabel;
use bsmr_query::query::syntax::simple::functions::QueryLiteralVisitor;
use bsmr_query_parser::Expr;
use bsmr_query_parser::spanned::Spanned;
use bsmr_util::arc_str::ArcSlice;
use bsmr_util::arc_str::ArcStr;

use super::coerced_attr::CoercedAttr;
use crate::attrs::coerced_path::CoercedPath;
use crate::configuration::resolved::ConfigurationSettingKey;

#[derive(bsmr_error::Error, Debug)]
#[bsmr(tag = Input)]
enum AttrCoercionContextError {
    #[error("Expected target label without name. Got `{0}`")]
    UnexpectedProvidersName(String),
}

/// The context for attribute coercion. Mostly just contains information about
/// the current package (to support things like parsing targets from strings).
pub trait AttrCoercionContext {
    fn coerce_target_label(&self, value: &str) -> bsmr_error::Result<TargetLabel> {
        let label = self.coerce_providers_label(value)?;

        if let ProvidersName::NonDefault(flavor) = label.name()
            && matches!(flavor.as_ref(), NonDefaultProvidersName::Named(_))
        {
            return Err(AttrCoercionContextError::UnexpectedProvidersName(value.to_owned()).into());
        }

        Ok(label.into_parts().0)
    }

    /// Attempt to convert a string into a label
    fn coerce_providers_label(&self, value: &str) -> bsmr_error::Result<ProvidersLabel>;

    /// Reuse previously allocated string if possible.
    fn intern_str(&self, value: &str) -> ArcStr;

    // Reuse previously allocated slices if possible.
    fn intern_list(&self, value: Vec<CoercedAttr>) -> ArcSlice<CoercedAttr>;

    // Reuse previously allocated selects if possible.
    fn intern_select(
        &self,
        value: Vec<(ConfigurationSettingKey, CoercedAttr)>,
    ) -> ArcSlice<(ConfigurationSettingKey, CoercedAttr)>;

    // Reuse previously allocated dicts if possible.
    fn intern_dict(
        &self,
        value: Vec<(CoercedAttr, CoercedAttr)>,
    ) -> ArcSlice<(CoercedAttr, CoercedAttr)>;

    /// Attempt to convert a string into a BuckPath
    fn coerce_path(&self, value: &str, allow_directory: bool) -> bsmr_error::Result<CoercedPath>;

    fn coerce_target_pattern(
        &self,
        pattern: &str,
    ) -> bsmr_error::Result<ParsedPattern<TargetPatternExtra>>;

    fn visit_query_function_literals<'q>(
        &self,
        visitor: &mut dyn QueryLiteralVisitor<'q>,
        expr: &Spanned<Expr<'q>>,
        query: &'q str,
    ) -> bsmr_error::Result<()>;
}
