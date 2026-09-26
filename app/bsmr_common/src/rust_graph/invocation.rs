//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Track the complete command selection before Cargo plans any selected entrypoint.

use std::sync::Arc;

use allocative::Allocative;
use bsmr_core::pattern::pattern_type::ConfiguredProvidersPatternExtra;
use bsmr_core::pattern::unparsed::UnparsedPatterns;
use dice::InjectedKey;
use dice::PagableValueSerialize;
use dice::ValueSerialize;
use pagable::Pagable;
use pagable::pagable_typetag;

/// Changing the requested roots invalidates feature resolution in a warm daemon.
#[derive(
    Clone,
    Debug,
    Eq,
    Hash,
    PartialEq,
    Allocative,
    Pagable,
    derive_more::Display
)]
#[display("CargoInvocation")]
#[pagable_typetag(dice::DiceKeyDyn)]
pub struct Invocation;

impl InjectedKey for Invocation {
    type Value = Arc<Option<UnparsedPatterns<ConfiguredProvidersPatternExtra>>>;

    /// Preserve a plan only while its complete root selection remains equal.
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }

    /// Keep the selection dependency when DICE pages computed plans.
    fn value_serialize() -> impl ValueSerialize<Value = Self::Value> {
        PagableValueSerialize::<Self::Value>::new()
    }
}
