//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Own the standard streams selected before a command starts.

use std::process::Stdio;

use crate::StdRedirectPaths;

/// Carry request descriptors without changing output capture or cancellation.
pub struct CommandIo {
    /// Consumed by process creation. The child owns its resulting input descriptor.
    pub stdin: Stdio,
    /// Absent redirects capture both output streams through the event stream.
    pub redirects: Option<StdRedirectPaths>,
}

impl Default for CommandIo {
    fn default() -> Self {
        Self {
            stdin: Stdio::null(),
            redirects: None,
        }
    }
}
