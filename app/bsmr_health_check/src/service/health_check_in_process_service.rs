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

#![allow(dead_code)] // Presently used only in oss

use bsmr_fs::paths::abs_norm_path::AbsNormPathBuf;

use crate::interface::HealthCheckContextEvent;
use crate::interface::HealthCheckService;
use crate::interface::HealthCheckSnapshotData;
use crate::report::Report;

pub struct HealthCheckInProcessService {}

impl HealthCheckInProcessService {
    pub fn new(_health_check_dir: AbsNormPathBuf) -> Self {
        Self {}
    }
}

#[async_trait::async_trait]
impl HealthCheckService for HealthCheckInProcessService {
    async fn update_context(&mut self, _event: HealthCheckContextEvent) -> bsmr_error::Result<()> {
        Ok(())
    }

    async fn run_checks(
        &mut self,
        _snapshot: HealthCheckSnapshotData,
    ) -> bsmr_error::Result<Vec<Report>> {
        Ok(Vec::new())
    }
}
