//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Names invalid or unsupported Cargo contracts before compilation.

use std::path::PathBuf;

use bsmr_core::cells::cell_path::CellPath;

/// Cargo graph input or an unsupported semantic boundary.
#[derive(Debug, bsmr_error::Error)]
#[bsmr(tag = Input)]
pub enum RustGraphError {
    #[error("Cargo directory `{0}` is not a workspace member")]
    NotWorkspaceMember(CellPath),
    #[error("invalid cargo metadata: {0}")]
    Metadata(#[source] serde_json::Error),
    #[error("native Rust import does not support {case} in `{package}`")]
    Unsupported {
        /// Package whose Cargo contract cannot yet be represented.
        package: String,
        /// Unsupported semantic requirement, safe to show to the user.
        case: String,
    },
    #[error("Cargo path `{0:?}` is outside the workspace")]
    Outside(PathBuf),
}

impl From<serde_json::Error> for RustGraphError {
    /// Preserve JSON decoding or serialization failure details.
    fn from(source: serde_json::Error) -> Self {
        Self::Metadata(source)
    }
}

/// Names the exact unsupported package contract.
pub(super) fn unsupported(package: &str, case: &str) -> RustGraphError {
    RustGraphError::Unsupported {
        package: package.to_owned(),
        case: case.to_owned(),
    }
}
