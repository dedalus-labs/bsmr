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

use std::collections::HashMap;

use bsmr_build_signals::env::CriticalPathBackendName;
use bsmr_build_signals::env::NodeDuration;
use bsmr_build_signals::env::WaitingData;
use bsmr_build_signals::error::CriticalPathError;
use bsmr_core::target::configured_target_label::ConfiguredTargetLabel;
use bsmr_events::span::SpanId;
use smallvec::SmallVec;

use crate::BuildInfo;
use crate::NodeExtraData;
use crate::NodeKey;

pub(crate) trait BuildListenerBackend {
    fn process_node(
        &mut self,
        key: NodeKey,
        extra_data: NodeExtraData,
        duration: NodeDuration,
        dep_keys: impl IntoIterator<Item = NodeKey>,
        span_ids: SmallVec<[SpanId; 1]>,
        waiting_data: WaitingData,
    );

    fn process_top_level_target(
        &mut self,
        analysis: ConfiguredTargetLabel,
        artifacts: impl IntoIterator<Item = NodeKey>,
    );

    fn finish(
        self,
        anon_target_discovery_edges: HashMap<NodeKey, NodeKey>,
    ) -> Result<BuildInfo, CriticalPathError>;

    fn name() -> CriticalPathBackendName;
}
