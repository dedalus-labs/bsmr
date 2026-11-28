//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies a locked official Go SDK from its build action output.

//! Uses the locked Go SDK where the verified archive action materialized it.
//!
//! The archive action authenticates the download against the lock, so the source tree
//! holds no SDK copy and no metadata that could drift from the lock.

use std::fs;
use std::path::Path;
use std::path::PathBuf;

use super::GoToolchainError;
use super::GoToolchainLock;
use super::legacy::DEMO_TOOLCHAINS;

/// Creates an absent `toolchains//` package directory with the default manifest.
///
/// An existing directory is left untouched, so a user's manifest or a cell aliased to the
/// project root keeps its own package definition.
pub(crate) fn create_toolchains_package(toolchains: &Path) -> Result<(), GoToolchainError> {
    match fs::read_dir(toolchains) {
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir_all(toolchains).map_err(|error| write_error(toolchains, error))?;
            let manifest = toolchains.join("BUILD.bsmr");
            fs::write(&manifest, DEMO_TOOLCHAINS).map_err(|error| write_error(&manifest, error))
        }
        Err(error) => Err(GoToolchainError::Read {
            path: toolchains.to_owned(),
            message: error.to_string(),
        }),
    }
}

/// Returns the `go` executable of a materialized SDK whose own version matches the lock.
pub(crate) fn sdk_go(sdk: &Path, lock: &GoToolchainLock) -> Result<PathBuf, GoToolchainError> {
    let path = sdk.join("VERSION");
    let declared = fs::read_to_string(&path).map_err(|error| GoToolchainError::Read {
        path,
        message: error.to_string(),
    })?;
    let declared = declared.lines().next().unwrap_or_default();
    if declared != format!("go{}", lock.version) {
        return Err(GoToolchainError::SdkVersion {
            locked: lock.version.clone(),
            declared: declared.to_owned(),
        });
    }
    Ok(sdk.join("bin/go"))
}

/// Converts an I/O error into one path-specific generated-file failure.
fn write_error(path: &Path, error: std::io::Error) -> GoToolchainError {
    GoToolchainError::Write {
        path: path.to_owned(),
        message: error.to_string(),
    }
}
