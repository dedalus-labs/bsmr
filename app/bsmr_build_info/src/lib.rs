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

pub struct BsmrBuildInfo {
    pub revision: Option<&'static str>,
    pub win_internal_version: Option<&'static str>,
    pub release_timestamp: Option<&'static str>,
}

pub static BSMR_BUILD_INFO: LateBinding<BsmrBuildInfo> = LateBinding::new("BSMR_BUILD_INFO");

/// Get the source control revision for this binary, if available. We provide this externally when
/// building Bessemer for release.
pub fn revision() -> Option<&'static str> {
    BSMR_BUILD_INFO
        .get()
        .ok()
        .and_then(|i| i.revision)
        .filter(|s| !s.is_empty())
}

/// Get the generated version for the windows binary. We use this for defining bucks internal version
pub fn win_internal_version() -> Option<&'static str> {
    BSMR_BUILD_INFO
        .get()
        .ok()
        .and_then(|i| i.win_internal_version)
        .filter(|s| !s.is_empty())
}

/// Get the time at which this binary was built, if available.
pub fn time_iso8601() -> Option<&'static str> {
    #[cfg(fbcode_build)]
    {
        Some(build_info::BuildInfo::get_time_iso8601())
    }

    #[cfg(not(fbcode_build))]
    {
        None
    }
}

/// A timestamp for this release. Notionally this is similar to time_iso8601, except a) the format
/// differs so it's more easily machine readable and b) we *only* set this in release binaries.
///
/// We use this in Ingress when dealing with panic and soft error reports to omit logging for older
/// versions.
pub fn release_timestamp() -> Option<&'static str> {
    BSMR_BUILD_INFO
        .get()
        .ok()
        .and_then(|i| i.release_timestamp)
        .filter(|s| !s.is_empty())
}
