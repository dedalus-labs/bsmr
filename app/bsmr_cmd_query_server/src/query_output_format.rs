//===----------------------------------------------------------------------===//
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

use bsmr_cli_proto::QueryOutputFormat;

#[derive(Debug, Clone)]
pub(crate) enum QueryOutputFormatInfo {
    Default,
    Json,
    Dot,
    DotCompact,
    Starlark,
}

impl QueryOutputFormatInfo {
    pub(crate) fn from_protobuf_int(value: i32) -> Option<Self> {
        let value = QueryOutputFormat::try_from(value).ok()?;
        let res = match value {
            QueryOutputFormat::Default => Self::Default,
            QueryOutputFormat::Json => Self::Json,
            QueryOutputFormat::Dot => Self::Dot,
            QueryOutputFormat::DotCompact => Self::DotCompact,
            QueryOutputFormat::Starlark => Self::Starlark,
            QueryOutputFormat::Html => return None,
        };
        Some(res)
    }
}
