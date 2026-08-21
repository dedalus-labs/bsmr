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

use dupe::Dupe;

#[derive(
    clap::ValueEnum,
    Dupe,
    Clone,
    Copy,
    Debug,
    serde::Serialize,
    serde::Deserialize
)]
pub enum BsmrProfileMode {
    TimeFlame,
    HeapAllocated,
    HeapRetained,
    HeapFlameAllocated,
    HeapFlameRetained,
    HeapSummaryAllocated,
    HeapSummaryRetained,
    Statement,
    Bytecode,
    BytecodePairs,
    Typecheck,
    Coverage,
    None,
}

impl BsmrProfileMode {
    pub fn to_proto(&self) -> bsmr_cli_proto::ProfileMode {
        match self {
            BsmrProfileMode::TimeFlame => bsmr_cli_proto::ProfileMode::TimeFlame,
            BsmrProfileMode::HeapAllocated => bsmr_cli_proto::ProfileMode::HeapAllocated,
            BsmrProfileMode::HeapRetained => bsmr_cli_proto::ProfileMode::HeapRetained,
            BsmrProfileMode::HeapFlameAllocated => bsmr_cli_proto::ProfileMode::HeapFlameAllocated,
            BsmrProfileMode::HeapFlameRetained => bsmr_cli_proto::ProfileMode::HeapFlameRetained,
            BsmrProfileMode::HeapSummaryAllocated => {
                bsmr_cli_proto::ProfileMode::HeapSummaryAllocated
            }
            BsmrProfileMode::HeapSummaryRetained => {
                bsmr_cli_proto::ProfileMode::HeapSummaryRetained
            }
            BsmrProfileMode::Statement => bsmr_cli_proto::ProfileMode::Statement,
            BsmrProfileMode::Bytecode => bsmr_cli_proto::ProfileMode::Bytecode,
            BsmrProfileMode::BytecodePairs => bsmr_cli_proto::ProfileMode::BytecodePairs,
            BsmrProfileMode::Typecheck => bsmr_cli_proto::ProfileMode::Typecheck,
            BsmrProfileMode::Coverage => bsmr_cli_proto::ProfileMode::Coverage,
            BsmrProfileMode::None => bsmr_cli_proto::ProfileMode::None,
        }
    }
}
