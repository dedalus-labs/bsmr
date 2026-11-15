//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Shares verified execution identity between analysis and action reuse.

use std::sync::Arc;

use allocative::Allocative;
use derive_more::Display;
use dice::InjectedKey;
use dice::PagableValueSerialize;
use dice::ValueSerialize;
use dupe::Dupe;
use pagable::Pagable;
use pagable::pagable_typetag;

/// Cache identity and native package admission must agree on this exact policy.
pub const NAMESPACE_PROFILE: &str = "declared-inputs-v3";

/// Invalidates analysis and completed actions when runtime bytes or policy change.
#[derive(Clone, Dupe, Display, Debug, Eq, Hash, PartialEq, Allocative, Pagable)]
#[display("{:?}", self)]
#[pagable_typetag(dice::DiceKeyDyn)]
pub struct ExecutionPlatformKey;

impl InjectedKey for ExecutionPlatformKey {
    type Value = Arc<Vec<(String, String)>>;

    /// Reuse a computation only when every verified execution property matches.
    fn equality(x: &Self::Value, y: &Self::Value) -> bool {
        x == y
    }

    /// Preserve the execution dependency when DICE pages its graph.
    fn value_serialize() -> impl ValueSerialize<Value = Self::Value> {
        PagableValueSerialize::<Self::Value>::new()
    }
}

/// Refreshes discovery after idle admission while overlapping commands share one observation.
#[derive(Clone, Dupe, Display, Debug, Eq, Hash, PartialEq, Allocative, Pagable)]
#[display("CommandEpoch")]
#[pagable_typetag(dice::DiceKeyDyn)]
pub struct CommandEpoch;

impl InjectedKey for CommandEpoch {
    type Value = ();

    /// Each idle admission starts a new observation of external state.
    fn equality(_: &Self::Value, _: &Self::Value) -> bool {
        false
    }

    /// Retain the command dependency when DICE pages its graph.
    fn value_serialize() -> impl ValueSerialize<Value = Self::Value> {
        PagableValueSerialize::<Self::Value>::new()
    }
}
