//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Serves one internal Cargo planning request without compiling source code.

mod acquisition;
mod flags;
mod git;
mod planning;
mod source;
mod types;

const CARGO_LIBRARY: &str = "0.98.0";

use std::io::Write;
use std::io::{self};

use anyhow::Result;

/// Read one request and write only the resulting graph to standard output.
fn main() -> Result<()> {
    let request: types::Request = serde_json::from_reader(io::stdin().lock())?;
    let _source_home = acquisition::lease(&request.cargo_home)?;
    let graph = planning::plan(request)?;
    let mut output = io::stdout().lock();
    serde_json::to_writer(&mut output, &graph)?;
    output.flush()?;
    Ok(())
}
