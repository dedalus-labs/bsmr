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

#![cfg(any(target_os = "macos", target_os = "linux"))]

use std::sync::OnceLock;

use bsmr_error::BsmrErrorContext;
use bsmr_error::bsmr_error;

#[allow(clippy::absurd_extreme_comparisons)]
pub fn sc_clk_tck() -> bsmr_error::Result<u32> {
    static TICKS: OnceLock<u32> = OnceLock::new();
    TICKS
        .get_or_try_init(|| unsafe {
            let rate = libc::sysconf(libc::_SC_CLK_TCK);
            let rate: u32 = rate
                .try_into()
                .bsmr_error_context("Integer overflow converting ticks per second")?;
            if rate <= 0 || rate > 10_000 {
                return Err(bsmr_error!(
                    bsmr_error::ErrorTag::CpuStats,
                    "Invalid ticks per second: {}",
                    rate
                ));
            }
            Ok(rate)
        })
        .copied()
}

#[cfg(test)]
mod tests {
    use crate::os::unix_like::sc_clk_tck::sc_clk_tck;

    #[test]
    fn test_ticks_per_second() {
        // It is always 100.
        assert_eq!(100, sc_clk_tck().unwrap());
    }
}
