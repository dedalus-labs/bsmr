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

use bsmr_node::attrs::attr_type::AttrType;
use bsmr_node::attrs::attr_type::visibility::VisibilityAttrType;
use bsmr_node::attrs::coerced_attr::CoercedAttr;
use bsmr_node::attrs::coercion_context::AttrCoercionContext;
use bsmr_node::attrs::configurable::AttrIsConfigurable;
use bsmr_node::visibility::StarlarkTargetNameGlob;
use bsmr_node::visibility::VisibilityPattern;
use bsmr_node::visibility::VisibilityWithinViewBuilder;
use starlark::typing::Ty;
use starlark::values::Value;
use starlark::values::ValueLike;

use crate::attrs::coerce::AttrTypeCoerce;
use crate::attrs::coerce::attr_type::AttrTypeExt;
use crate::attrs::coerce::attr_type::list::coerce_list;
use crate::attrs::coerce::attr_type::ty_maybe_select::TyMaybeSelect;
use crate::interpreter::selector::StarlarkSelector;

#[derive(Debug, bsmr_error::Error)]
enum VisibilityAttrTypeCoerceError {
    #[error("Visibility attribute is not configurable (internal error)")]
    #[bsmr(tag = Tier0)]
    AttrTypeNotConfigurable,
    #[error("Visibility must be a list of strings or target_name_glob values, got `{0}`")]
    #[bsmr(tag = Input)]
    WrongType(String),
    #[error("Visibility attribute is not configurable (i.e. cannot use `select()`): `{0}`")]
    #[bsmr(tag = Input)]
    NotConfigurable(String),
}

impl AttrTypeCoerce for VisibilityAttrType {
    fn coerce_item(
        &self,
        configurable: AttrIsConfigurable,
        ctx: &dyn AttrCoercionContext,
        value: Value,
    ) -> bsmr_error::Result<CoercedAttr> {
        if configurable == AttrIsConfigurable::Yes {
            return Err(VisibilityAttrTypeCoerceError::AttrTypeNotConfigurable.into());
        }
        Ok(CoercedAttr::Visibility(
            parse_visibility_with_view(ctx, value)?.build_visibility(),
        ))
    }

    fn starlark_type(&self) -> TyMaybeSelect {
        TyMaybeSelect::List(Box::new(TyMaybeSelect::Union(vec![
            AttrType::string().starlark_type(),
            TyMaybeSelect::Basic(Ty::starlark_value::<StarlarkTargetNameGlob>()),
        ])))
    }
}

pub(crate) fn parse_visibility_with_view(
    ctx: &dyn AttrCoercionContext,
    attr: Value,
) -> bsmr_error::Result<VisibilityWithinViewBuilder> {
    let list = match coerce_list(attr) {
        Ok(list) => list,
        Err(e) => {
            if StarlarkSelector::from_value(attr).is_some() {
                return Err(VisibilityAttrTypeCoerceError::NotConfigurable(attr.to_repr()).into());
            }
            return Err(e);
        }
    };

    let mut builder = VisibilityWithinViewBuilder::with_capacity(list.len());
    for item in list {
        if let Some(s) = item.unpack_str() {
            if s == VisibilityPattern::PUBLIC {
                builder.add_public();
            } else {
                builder.add(VisibilityPattern::Parsed(ctx.coerce_target_pattern(s)?));
            }
        } else if let Some(target_name_glob) = item.downcast_ref::<StarlarkTargetNameGlob>() {
            let record = target_name_glob.coerce(|s| ctx.coerce_target_pattern(s))?;
            builder.add(VisibilityPattern::TargetNameGlob(record));
        } else {
            if StarlarkSelector::from_value(*item).is_some() {
                return Err(VisibilityAttrTypeCoerceError::NotConfigurable(item.to_repr()).into());
            }
            return Err(VisibilityAttrTypeCoerceError::WrongType(item.to_repr()).into());
        }
    }
    Ok(builder)
}
