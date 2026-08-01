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

macro_rules! late_binding_ty {
    ($name:ident, $late:ident) => {
        static $late: bsmr_util::late_binding::LateBinding<starlark::typing::Ty> =
            bsmr_util::late_binding::LateBinding::new(stringify!($name));

        #[allow(clippy::empty_enums)]
        pub enum $name {}

        impl $name {
            pub fn init(ty: starlark::typing::Ty) {
                $late.init(ty);
            }
        }

        impl starlark::values::type_repr::StarlarkTypeRepr for $name {
            type Canonical = Self;

            fn starlark_type_repr() -> starlark::typing::Ty {
                dupe::Dupe::dupe($late.get().unwrap())
            }
        }
    };
}

late_binding_ty!(AnalysisContextReprLate, ANALYSIS_CONTEXT_REPR_LATE);
late_binding_ty!(ProviderReprLate, PROVIDER_REPR_LATE);
late_binding_ty!(TransitionReprLate, TRANSITION_REPR_LATE);
