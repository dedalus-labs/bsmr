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

//! Protobufs for ineteracting with Bsmr's DownwardApi over GPRC. This isn't the protocol Bsmr v1
//! speaks, where the DownwardApi is accessed over named pipes with serialized JSON payloads. This
//! is a different way to make the same calls.

// We put this in a module for easier naming in convert.
mod proto {
    tonic::include_proto!("bsmr.downward_api");
}

pub use proto::*;

mod convert;
