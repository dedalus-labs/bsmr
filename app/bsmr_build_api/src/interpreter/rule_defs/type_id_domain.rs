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

//! bsmr's [`TypeIdDomain`]s (providers, transitive sets). Kept in bsmr rather
//! than starlark-rust, which is bsmr-agnostic and only knows its own
//! `record`/`enum` domains.

use dupe::Dupe;
use starlark::values::typing::TypeIdDomain;

/// [`TypeIdDomain`]s for bsmr-defined nominal types.
#[derive(Copy, Clone, Dupe, Debug, Eq, PartialEq)]
pub(crate) enum BsmrTypeIdDomain {
    /// A user-defined `provider(...)` type.
    UserProvider,
    /// A transitive set definition.
    TransitiveSet,
    /// A builtin (Rust-defined) provider type — both its instance and callable
    /// types; the role is disambiguated by the identity passed to `from_identity`.
    BuiltinProvider,
    /// The singleton `Provider` type that matches any provider instance.
    ProviderSingleton,
}

impl TypeIdDomain for BsmrTypeIdDomain {
    fn tag(&self) -> &'static str {
        match self {
            BsmrTypeIdDomain::UserProvider => "bsmr.user_provider",
            BsmrTypeIdDomain::TransitiveSet => "bsmr.transitive_set",
            BsmrTypeIdDomain::BuiltinProvider => "bsmr.builtin_provider",
            BsmrTypeIdDomain::ProviderSingleton => "bsmr.provider_singleton",
        }
    }
}
