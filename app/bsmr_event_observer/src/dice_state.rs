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

use std::collections::BTreeMap;

use bsmr_data::DiceKeyState;
use bsmr_data::DiceStateSnapshot;

pub struct DiceState {
    key_states: BTreeMap<String, DiceKeyState>,
}

impl DiceState {
    pub fn new() -> Self {
        Self {
            key_states: BTreeMap::new(),
        }
    }

    pub fn update(&mut self, update: &DiceStateSnapshot) {
        for (k, v) in &update.key_states {
            self.key_states.insert(k.clone(), *v);
        }
    }

    pub fn key_states(&self) -> &BTreeMap<String, DiceKeyState> {
        &self.key_states
    }
}
