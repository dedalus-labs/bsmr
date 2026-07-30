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

#[cfg(test)]
mod attr;
mod attrs;
mod functions;
pub mod interpreter;
mod label;
mod rule;
pub mod select;
mod super_package;
mod tests;
mod uncategorized;

#[test]
fn init_late_bindings_for_test() {
    #[ctor::ctor(unsafe)]
    fn init() {
        bsmr_interpreter_for_build::init_late_bindings();
        bsmr_build_api::init_late_bindings();
        bsmr_transition::init_late_bindings();
    }
}
