//===----------------------------------------------------------------------===//
// Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc
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

//! Contains utilities for dealing with buckv1 concepts (ex. buckv1's
//! .bsmrconfig files as configuration)

mod access;
pub use access::parse_bsmrconfig_metadata;
mod aggregator;
pub mod args;
pub mod cells;
pub mod configs;
pub mod dice;
pub mod file_ops;
pub mod key;
mod parser;
pub(crate) mod path;
pub mod view;
