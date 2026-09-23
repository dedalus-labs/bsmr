//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Imports Cargo workspace descriptions through native Rust rules.
//!
//! # Cargo cache boundary
//!
//! BSMR's native Rust cache contract assigns cross-workspace action identity,
//! atomic publication, locking, and collection to BSMR until Cargo ships
//! equivalent native behavior. Track the complete upstream boundary:
//!
//! - goal and design: <https://github.com/rust-lang/goals/issues/626> and
//!   <https://github.com/rust-lang/cargo/issues/5931>
//! - publication prototypes: <https://github.com/ranger-ross/cargo/pull/29>,
//!   <https://github.com/ranger-ross/cargo/pull/30>, and
//!   <https://github.com/ranger-ross/cargo/pull/31>
//! - landed layout and locking: <https://github.com/rust-lang/cargo/pull/15947>,
//!   <https://github.com/rust-lang/cargo/pull/17354>, and
//!   <https://github.com/rust-lang/cargo/pull/16155>
//! - remaining locking, legacy removal, and garbage collection:
//!   <https://github.com/rust-lang/cargo/issues/4282>,
//!   <https://github.com/rust-lang/cargo/issues/17182>, and
//!   <https://github.com/rust-lang/cargo/issues/5026>

pub mod catalog;
pub mod configured;
pub mod dice;
pub mod entry;
mod error;
mod planner;
mod snapshot;
mod toolchain;
mod units;

pub use error::RustGraphError;
use error::unsupported;
