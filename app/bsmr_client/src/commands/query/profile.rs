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

use bsmr_client_ctx::common::profiling::BuckProfileMode;
use bsmr_client_ctx::path_arg::PathArg;

/// Starlark profiling options
#[derive(Debug, Clone, clap::Parser)]
#[clap(next_help_heading = "Starlark Profiling Options")]
pub(crate) struct QueryProfileOptions {
    /// Profile target loading.
    ///
    /// When this option is enabled, Buck will profile every `BUCK` file loaded during the query
    /// and merge the results into a single profile.
    /// The command may return cached profile data if `BUCK` files were not invalidated.
    #[clap(long, requires("profile_output"))]
    pub(crate) profile_mode: Option<BuckProfileMode>,

    /// Where to write profile output.
    #[clap(long)]
    pub(crate) profile_output: Option<PathArg>,
}

impl QueryProfileOptions {
    pub(crate) fn profile_mode_proto(&self) -> Option<bsmr_cli_proto::ProfileMode> {
        self.profile_mode.map(|v| v.to_proto())
    }
}
