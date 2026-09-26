//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Imports Cargo workspace descriptions through native Rust rules.

pub mod catalog;
pub mod checkout;
pub mod configured;
pub mod dice;
pub mod entry;
mod error;
pub mod git;
pub mod invocation;
mod libraries;
mod planner;
pub(crate) mod project;
mod selection;
mod snapshot;
mod sources;
mod toolchain;
mod units;

pub use error::RustGraphError;
use error::unsupported;
