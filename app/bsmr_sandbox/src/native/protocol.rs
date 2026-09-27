//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Describe the native worker without importing privileged implementation code.

use serde::Deserialize;
use serde::Serialize;

use crate::GuestAction;

/// Untrusted wire data. The worker accepts it only through `Request::read`.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Wire {
    /// Installed runtime identity supplied by the unprivileged executor.
    pub environment: String,
    /// SHA-256 of the complete input archive passed on the second descriptor.
    pub input: String,
    /// Existing BSMR action schema, shared with the other execution backends.
    pub action: GuestAction,
}

/// Published before admission so the executor can bind its cache key to actual bytes.
#[derive(Deserialize, Serialize)]
pub struct Info {
    /// Protocol understood by the installed worker.
    pub protocol: u32,
    /// Digest of the retained runtime snapshot and trusted launcher.
    pub environment: String,
    /// Maximum simultaneous executions in this worker.
    pub slots: usize,
}
