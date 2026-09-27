//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Native macOS process ownership for the privileged build supervisor.

#[cfg(unix)]
pub mod archive;

#[cfg(unix)]
pub mod request;

#[cfg(unix)]
pub mod workspace;

#[cfg(target_os = "macos")]
pub mod identity;

#[cfg(target_os = "macos")]
pub mod job;

#[cfg(target_os = "macos")]
pub mod output;

#[cfg(target_os = "macos")]
pub mod launch;

#[cfg(target_os = "macos")]
pub mod run;

#[cfg(target_os = "macos")]
pub mod runtime;

#[cfg(target_os = "macos")]
pub mod worker;
