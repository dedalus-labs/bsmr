//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Native macOS process ownership for the privileged build supervisor.

#[cfg(target_os = "macos")]
pub mod identity;

#[cfg(target_os = "macos")]
pub mod run;

#[cfg(target_os = "macos")]
pub mod wait;
