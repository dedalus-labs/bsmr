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

use bsmr_util::late_binding::LateBinding;

/// A trait that exposes only the bsmr_data-dependent bits of an EventDispatcher.
///
/// This is so we can use a `LateBinding` and dispatch events from bsmr_core,
/// which can't take a depndency on bsmr_event (where `EventDispatcher` is defined).
pub trait EventDispatch: Send + Sync {
    fn emit_instant_event_for_data(&self, data: bsmr_data::instant_event::Data);
}

pub static EVENT_DISPATCH: LateBinding<&'static dyn EventDispatch> =
    LateBinding::new("EVENT_DISPATCH");
