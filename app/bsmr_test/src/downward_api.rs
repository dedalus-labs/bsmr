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

use async_trait::async_trait;
use bsmr_downward_api::DownwardApi;
use bsmr_hash::StdBsmrHashMap;
use tracing::Level;

pub struct BsmrTestDownwardApi;

#[async_trait]
impl DownwardApi for BsmrTestDownwardApi {
    async fn console(&self, _level: Level, msg: String) -> bsmr_error::Result<()> {
        // TODO(brasselsprouts): use the level and hook it up with our superconsole
        eprintln!("{}", msg);
        Ok(())
    }

    async fn log(&self, _level: Level, _msg: String) -> bsmr_error::Result<()> {
        unimplemented!("TODO(bobyf)")
    }

    async fn external(&self, _data: StdBsmrHashMap<String, String>) -> bsmr_error::Result<()> {
        unimplemented!("need bsmr event stream to implement")
    }
}
