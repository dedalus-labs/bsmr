//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Stage verified input files directly without the VM archive transport.

use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::fs::FileTimes;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::time::UNIX_EPOCH;

use bsmr_common::file_ops::metadata::FileMetadata;
use bsmr_directory::directory::directory::Directory;
use bsmr_directory::directory::directory_iterator::DirectoryIterator;
use bsmr_directory::directory::entry::DirectoryEntry;
use bsmr_execute::digest_config::DigestConfig;
use bsmr_execute::directory::ActionDirectoryMember;
use bsmr_execute::directory::ActionImmutableDirectory;
use bsmr_fs::error::IoResultExt;
use bsmr_fs::fs_util;
use bsmr_fs::paths::abs_path::AbsPath;
use parking_lot::Mutex;

use crate::executors::firecracker;
use crate::executors::inputs;

#[derive(Debug, bsmr_error::Error)]
#[bsmr(tag = Input)]
enum StagingError {
    #[error("namespace input path exceeds its depth limit: {0:?}")]
    Depth(PathBuf),
    #[error("namespace input symlink {path:?} -> {target:?} escapes the action root")]
    Symlink {
        /// Declared link inside the private input tree.
        path: PathBuf,
        /// Target that would escape the action's filesystem.
        target: PathBuf,
    },
}

/// Own verified files shared only by read-only mounts within one command.
pub(super) struct Cache {
    /// Private storage on the action output filesystem, so linking cannot cross devices.
    directory: tempfile::TempDir,
    /// Only complete, verified files enter this index. Permissions are part of the key.
    files: Mutex<HashMap<FileMetadata, PathBuf>>,
}

impl Cache {
    /// Keep the verified copies separate from the project's mutable files.
    pub(super) fn new(parent: &Path) -> bsmr_error::Result<Self> {
        Ok(Self {
            directory: tempfile::Builder::new()
                .prefix(".bsmr-inputs-")
                .tempdir_in(parent)?,
            files: Mutex::new(HashMap::new()),
        })
    }

    /// Create a private input snapshot whose files match the analyzed action digests.
    pub(super) fn stage(
        &self,
        project: &Path,
        root: &Path,
        directory: &ActionImmutableDirectory,
        digest_config: DigestConfig,
    ) -> bsmr_error::Result<()> {
        fs::create_dir(root)?;
        let mut directories = Vec::new();
        for (path, entry) in directory.ordered_walk().with_paths() {
            let path = Path::new(path.as_str());
            firecracker::validate_guest_path(path)?;
            if path.components().count() > firecracker::ARCHIVE_PATH_DEPTH_LIMIT {
                return Err(StagingError::Depth(path.to_owned()).into());
            }
            let destination = root.join(path);
            match entry {
                DirectoryEntry::Dir(_) => {
                    fs::create_dir(&destination)?;
                    directories.push(destination);
                }
                DirectoryEntry::Leaf(ActionDirectoryMember::File(metadata)) => {
                    self.copy_file(project, root, path, metadata, digest_config)?;
                }
                DirectoryEntry::Leaf(ActionDirectoryMember::Symlink(link)) => {
                    let target = Path::new(link.target().as_str());
                    if !firecracker::relative_symlink_stays_within(path, target, Path::new("")) {
                        return Err(StagingError::Symlink {
                            path: path.to_owned(),
                            target: target.to_owned(),
                        }
                        .into());
                    }
                    fs_util::symlink(target, AbsPath::new(&destination)?).categorize_internal()?;
                }
                DirectoryEntry::Leaf(ActionDirectoryMember::ExternalSymlink(link)) => {
                    return Err(StagingError::Symlink {
                        path: path.to_owned(),
                        target: link.to_path_buf(),
                    }
                    .into());
                }
            }
        }
        // Child creation changes directory mtimes, so normalize them after all writes.
        for path in directories.into_iter().rev() {
            normalize(&File::open(path)?, true)?;
        }
        Ok(())
    }

    /// Preserve the archive transport's contents, executable bit and normalized mtime.
    fn copy_file(
        &self,
        project: &Path,
        root: &Path,
        path: &Path,
        metadata: &FileMetadata,
        digest_config: DigestConfig,
    ) -> bsmr_error::Result<()> {
        let mut files = self.files.lock();
        if let Some(verified) = files.get(metadata) {
            fs::hard_link(verified, root.join(path))?;
            return Ok(());
        }
        let verified = self.directory.path().join(files.len().to_string());
        let mut file = File::create(&verified)?;
        inputs::with_file(project, path, metadata, digest_config, |reader| {
            std::io::copy(&mut reader.take(metadata.digest.size()), &mut file)
        })?;
        normalize(&file, metadata.is_executable)?;
        fs::hard_link(&verified, root.join(path))?;
        files.insert(metadata.clone(), verified);
        Ok(())
    }
}

/// Match the original transport's executable modes and epoch modification times.
fn normalize(file: &File, executable: bool) -> bsmr_error::Result<()> {
    #[cfg(not(unix))]
    let _ = executable;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = if executable { 0o755 } else { 0o644 };
        file.set_permissions(fs::Permissions::from_mode(mode))?;
    }
    file.set_times(FileTimes::new().set_modified(UNIX_EPOCH))?;
    Ok(())
}

#[cfg(all(test, unix))]
#[path = "inputs/tests.rs"]
mod tests;
