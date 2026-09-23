//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Imports Cargo workspace descriptions through native Rust rules.

pub mod catalog;
pub mod configured;
pub mod dice;
pub mod entry;
mod error;
mod planner;
mod selection;
mod snapshot;
mod sources;
mod toolchain;
mod units;

pub use error::RustGraphError;
use error::unsupported;
