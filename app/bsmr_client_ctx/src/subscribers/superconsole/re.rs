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

use bsmr_event_observer::re_state::ReState;
use bsmr_event_observer::two_snapshots::TwoSnapshots;
use superconsole::Component;

use crate::subscribers::superconsole::SuperConsoleConfig;

/// Draw the test summary line above the `timed_list`
pub(crate) struct ReHeader<'a> {
    pub(crate) super_console_config: &'a SuperConsoleConfig,
    pub(crate) re_state: &'a ReState,
    pub(crate) two_snapshots: &'a TwoSnapshots,
}

impl Component for ReHeader<'_> {
    type Error = bsmr_error::Error;

    fn draw_unchecked(
        &self,
        _dimensions: superconsole::Dimensions,
        mode: superconsole::DrawMode,
    ) -> bsmr_error::Result<superconsole::Lines> {
        self.re_state.render(
            self.two_snapshots,
            self.super_console_config.enable_detailed_re,
            mode,
        )
    }
}
