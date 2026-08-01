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

use starlark::values::FrozenHeapName;
use strong_hash::StrongHash;

/// Testing sentinel for bsmr test code.
/// Used as `FrozenHeapName::User(Box::new(BsmrTestHeapName))`.
#[derive(Clone, derive_more::Display, Debug, Hash, StrongHash)]
#[display("BsmrTestHeapName")]
pub struct BsmrTestHeapName;

impl BsmrTestHeapName {
    pub fn frozen_heap_name() -> FrozenHeapName {
        FrozenHeapName::User(Box::new(Self))
    }
}
