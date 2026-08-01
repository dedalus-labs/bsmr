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

#![cfg(test)]
#![allow(clippy::bool_assert_comparison)]

mod actions;
mod analysis;
mod artifact_groups;
mod attrs;
mod build;
mod interpreter;
mod nodes;

#[test]
fn init_late_bindings_for_test() {
    #[ctor::ctor(unsafe)]
    fn init() {
        bsmr_action_impl::init_late_bindings();
        bsmr_analysis::init_late_bindings();
        bsmr_anon_target::init_late_bindings();
        bsmr_configured::init_late_bindings();
        bsmr_events::init_late_bindings();
        bsmr_interpreter_for_build::init_late_bindings();
        bsmr_build_api::init_late_bindings();
        bsmr_transition::init_late_bindings();
    }
}
