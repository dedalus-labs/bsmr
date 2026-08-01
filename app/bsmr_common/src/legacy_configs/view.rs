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

use std::fmt::Debug;
use std::str::FromStr;
use std::sync::Arc;

use crate::legacy_configs::configs::LegacyBsmrConfig;
use crate::legacy_configs::key::BsmrconfigKeyRef;

/// Bsmrconfig trait.
///
/// There are two implementations:
/// * simple implementation which is backed by a bsmrconfig object, used in tests
/// * DICE-backed implementation which records a dependency on bsmrconfig property in DICE
pub trait LegacyBsmrConfigView: Debug {
    fn get(&mut self, key: BsmrconfigKeyRef) -> bsmr_error::Result<Option<Arc<str>>>;

    fn parse<T: FromStr>(&mut self, key: BsmrconfigKeyRef) -> bsmr_error::Result<Option<T>>
    where
        bsmr_error::Error: From<<T as FromStr>::Err>,
    {
        LegacyBsmrConfig::parse_value(key, self.get(key)?.as_deref())
    }

    fn parse_list<T: FromStr>(
        &mut self,
        key: BsmrconfigKeyRef,
    ) -> bsmr_error::Result<Option<Vec<T>>>
    where
        bsmr_error::Error: From<<T as FromStr>::Err>,
    {
        LegacyBsmrConfig::parse_list_value(key, self.get(key)?.as_deref())
    }
}
