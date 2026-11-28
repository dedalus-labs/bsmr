//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Reads the exact Go SDK pin from a module or workspace file.

//! Delegates Go module/workspace pin parsing to `bsmr_common::native_toolchains`
//! so the CLI and DICE package evaluation share one exact parser.

use std::fs;
use std::path::Path;

use bsmr_common::native_toolchains::GoPinParseError;
use bsmr_common::native_toolchains::parse_go_pin;

use super::GoToolchainError;

/// One exact stable Go release pin and the root file that declared it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ModulePin {
    /// Canonical `1.X.Y` release version without Go's `go` prefix.
    version: String,
    /// Root file name (`go.work` or `go.mod`) that supplied the pin.
    file: &'static str,
}

impl ModulePin {
    /// Returns the canonical `1.X.Y` release version.
    pub(crate) fn version(&self) -> &str {
        &self.version
    }

    /// Returns the root file name (`go.work` or `go.mod`) that supplied the pin.
    pub(crate) fn file(&self) -> &'static str {
        self.file
    }
}

/// Returns the Go pin file name (`go.work` when present, otherwise `go.mod`).
pub(crate) fn pin_file_name(root: &Path) -> &'static str {
    if root.join("go.work").is_file() {
        "go.work"
    } else {
        "go.mod"
    }
}

/// Reads the exact stable pin from `go.work` when present, otherwise `go.mod`.
pub(crate) fn module_pin(root: &Path) -> Result<ModulePin, GoToolchainError> {
    let file = pin_file_name(root);
    let path = root.join(file);
    let source = fs::read_to_string(&path).map_err(|error| GoToolchainError::Read {
        path: path.clone(),
        message: error.to_string(),
    })?;
    let version = parse_go_pin(&source)
        .map_err(|error| match error {
            GoPinParseError::InvalidPin => GoToolchainError::ModulePin(path.clone()),
            GoPinParseError::Syntax(message) => GoToolchainError::ModuleSyntax {
                path: path.clone(),
                message,
            },
        })?
        .ok_or(GoToolchainError::ModulePin(path))?;
    Ok(ModulePin {
        version: version.to_owned(),
        file,
    })
}
