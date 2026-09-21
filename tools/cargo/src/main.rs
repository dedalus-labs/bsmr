//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Serves one internal Cargo planning request without compiling source code.

mod flags;
mod planning;
mod types;

use std::io;

use anyhow::Result;

/// Read one request and write only the resulting graph to standard output.
fn main() -> Result<()> {
    let request: types::Request = serde_json::from_reader(io::stdin().lock())?;
    planning::plan(request)
}
