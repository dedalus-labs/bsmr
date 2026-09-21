//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Imports Cargo workspace descriptions through native Rust rules.

pub mod dice;
mod metadata;
mod snapshot;
mod toolchain;

pub use metadata::RustGraphError;
pub use metadata::render;
use metadata::unsupported;
