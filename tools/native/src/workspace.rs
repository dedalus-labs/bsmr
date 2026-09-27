//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Prepare writable output parents without making declared input files mutable.

use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::archive::{self, Archive};
use crate::request::Request;

/// A prepared workspace retained until its action has stopped and outputs are validated.
pub struct Workspace(PathBuf);

/// Preparation never changes a path outside the supervisor's private workspace.
#[derive(Debug, Error)]
pub enum Error {
    #[error("native output overlaps a declared input or link: {0:?}")]
    Overlap(PathBuf),
    #[error("native workspace input failed: {0}")]
    Archive(#[from] archive::Error),
    #[error("native workspace I/O failed: {0}")]
    Io(#[from] io::Error),
}

impl Workspace {
    /// Expand verified inputs under a new private root, then allow declared outputs.
    ///
    /// `root` is supervisor-owned and has no running writers. The supervisor owns
    /// cleanup. On macOS it must run as root, so input ownership differs from the
    /// leased action UID. Sticky output parents protect existing input entries.
    pub fn prepare(root: &Path, request: &Request, input: &mut Archive) -> Result<Self, Error> {
        let workspace = Self(root.join("workspace"));
        fs::create_dir(&workspace.0)?;
        input.unpack(&workspace.0)?;
        for output in &request.action().outputs {
            workspace.parent(&output.path)?;
            match fs::symlink_metadata(workspace.0.join(&output.path)) {
                Ok(_) => return Err(Error::Overlap(output.path.clone())),
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        if let Some(scratch) = request.action().environment.get("BSMR_SCRATCH_PATH") {
            let scratch = Path::new(scratch);
            workspace.parent(scratch)?;
            let path = workspace.0.join(scratch);
            match fs::symlink_metadata(&path) {
                // The engine declares scratch as an empty input directory.
                // Existing files or aliases never become writable scratch.
                Ok(metadata)
                    if metadata.is_dir() && fs::read_dir(&path)?.next().transpose()?.is_none() => {}
                Ok(_) => return Err(Error::Overlap(scratch.to_owned())),
                Err(error) if error.kind() == io::ErrorKind::NotFound => fs::create_dir(&path)?,
                Err(error) => return Err(error.into()),
            }
            fs::set_permissions(path, fs::Permissions::from_mode(0o1777))?;
        }
        Ok(workspace)
    }

    /// Return the host-side path. Action processes see it as `/workspace`.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.0
    }

    /// Require real directories before granting write access to any output parent.
    fn parent(&self, output: &Path) -> Result<(), Error> {
        let mut path = self.0.clone();
        let parent = output
            .parent()
            .expect("validated relative output has a parent");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o1777))?;
        for component in parent.components() {
            path.push(component);
            match fs::symlink_metadata(&path) {
                Ok(metadata) if metadata.is_dir() => {}
                Ok(_) => return Err(Error::Overlap(path)),
                Err(error) if error.kind() == io::ErrorKind::NotFound => fs::create_dir(&path)?,
                Err(error) => return Err(error.into()),
            }
            fs::set_permissions(&path, fs::Permissions::from_mode(0o1777))?;
        }
        Ok(())
    }
}
