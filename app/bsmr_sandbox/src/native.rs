//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Shared native worker protocol and authenticated client transport.

#[cfg(target_os = "macos")]
pub mod client;
#[cfg(target_os = "macos")]
pub mod files;
pub mod protocol;
