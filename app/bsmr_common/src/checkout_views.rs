//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Owns one managed output view for each canonical BSMR project root.

use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::io;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use bsmr_fs::paths::abs_norm_path::AbsNormPath;
use serde::Deserialize;
use serde::Serialize;
use sha2::Digest;
use sha2::Sha256;

const OUTPUT_ROOT: &str = "bsmr-out";
const RECORD_VERSION: u8 = 1;
const VIEWS_VERSION: &str = "v1";

#[derive(Debug, bsmr_error::Error)]
#[bsmr(tag = Environment)]
enum CheckoutViewError {
    #[error("Checkout view {operation} failed for '{}': {source}", path.display())]
    Io {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("Checkout view root must be absolute, got '{}'", _0.display())]
    RelativeRoot(PathBuf),
    #[error("Could not decode checkout view record '{}': {source}", path.display())]
    DecodeRecord {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("Existing output root '{}' is not a BSMR-managed link", _0.display())]
    UnmanagedOutputRoot(PathBuf),
    #[error("Checkout view entry '{}' is not a regular file", _0.display())]
    UnmanagedEntry(PathBuf),
    #[error(
        "Managed output link '{}' targets '{}', expected '{}'",
        path.display(),
        actual.display(),
        expected.display()
    )]
    LinkMismatch {
        path: PathBuf,
        actual: PathBuf,
        expected: PathBuf,
    },
    #[error("System clock is earlier than the Unix epoch: {0}")]
    SystemTime(#[source] std::time::SystemTimeError),
}

#[derive(Deserialize, Serialize)]
struct ViewRecord {
    version: u8,
    project_root: PathBuf,
    updated_secs: u64,
}

/// Manages output directories independently from immutable action-cache identity.
pub struct CheckoutViews {
    root: PathBuf,
}

/// Holds shared ownership of one checkout view until its daemon exits.
#[derive(Debug)]
pub struct CheckoutViewLease {
    file: File,
}

impl CheckoutViews {
    /// Opens one absolute machine-level checkout-view root.
    pub fn at(root: PathBuf) -> bsmr_error::Result<Self> {
        if !root.is_absolute() {
            return Err(CheckoutViewError::RelativeRoot(root).into());
        }
        Ok(Self { root })
    }

    /// Creates or validates one managed output link and holds its shared lease.
    pub fn prepare(&self, project_root: &AbsNormPath) -> bsmr_error::Result<CheckoutViewLease> {
        let project_root: &Path = project_root.as_ref();
        let key = view_key(project_root);
        let view_root = self.root.join(VIEWS_VERSION);
        let view = view_root.join(&key);
        let record = view_root.join(format!("{key}.json"));
        let lock = view_root.join(format!("{key}.lock"));
        fs::create_dir_all(&self.root)
            .map_err(|error| io_error("create checkout view root", &self.root, error))?;
        validate_directory(&self.root)?;
        fs::create_dir_all(&view_root)
            .map_err(|error| io_error("create view root", &view_root, error))?;
        validate_directory(&view_root)?;
        let registry_lock = view_root.join("registry.lock");
        let registry = open_lock(&registry_lock)?;
        fs4::fs_std::FileExt::lock_exclusive(&registry)
            .map_err(|error| io_error("lock view registry", &registry_lock, error))?;
        let file = open_lock(&lock)?;
        fs4::fs_std::FileExt::lock_shared(&file)
            .map_err(|error| io_error("lease", &lock, error))?;

        validate_link(&project_root.join(OUTPUT_ROOT), &view)?;
        fs::create_dir_all(&view).map_err(|error| io_error("create", &view, error))?;
        ensure_link(&project_root.join(OUTPUT_ROOT), &view)?;
        write_record(&record, project_root)?;

        fs4::fs_std::FileExt::unlock(&registry)
            .map_err(|error| io_error("unlock view registry", &registry_lock, error))?;
        Ok(CheckoutViewLease { file })
    }
}

impl Drop for CheckoutViewLease {
    fn drop(&mut self) {
        if let Err(error) = fs4::fs_std::FileExt::unlock(&self.file) {
            tracing::warn!(%error, "checkout view lease release failed");
        }
    }
}

/// Writes the checkout record while the exclusive view lock is held.
fn write_record(path: &Path, project_root: &Path) -> bsmr_error::Result<()> {
    let record = ViewRecord {
        version: RECORD_VERSION,
        project_root: project_root.to_owned(),
        updated_secs: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(CheckoutViewError::SystemTime)?
            .as_secs(),
    };
    let mut contents =
        serde_json::to_vec(&record).map_err(|source| CheckoutViewError::DecodeRecord {
            path: path.to_owned(),
            source,
        })?;
    contents.push(b'\n');
    let parent = path
        .parent()
        .expect("checkout record is always beneath the view root");
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .map_err(|error| io_error("create record temporary", path, error))?;
    temporary
        .write_all(&contents)
        .map_err(|error| io_error("write record temporary", path, error))?;
    temporary
        .persist(path)
        .map_err(|error| io_error("publish record", path, error.error))?;
    Ok(())
}

/// Creates the visible output link or verifies the exact managed target.
fn ensure_link(path: &Path, expected: &Path) -> bsmr_error::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            let actual = fs::read_link(path).map_err(|error| io_error("read link", path, error))?;
            if actual == expected {
                return Ok(());
            }
            Err(CheckoutViewError::LinkMismatch {
                path: path.to_owned(),
                actual,
                expected: expected.to_owned(),
            }
            .into())
        }
        Ok(_) => Err(CheckoutViewError::UnmanagedOutputRoot(path.to_owned()).into()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            create_dir_symlink(expected, path).map_err(|error| io_error("link", path, error))
        }
        Err(error) => Err(io_error("inspect link", path, error)),
    }
}

/// Rejects every pre-existing output root except the exact managed symlink.
fn validate_link(path: &Path, expected: &Path) -> bsmr_error::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            let actual = fs::read_link(path).map_err(|error| io_error("read link", path, error))?;
            if actual == expected {
                Ok(())
            } else {
                Err(CheckoutViewError::LinkMismatch {
                    path: path.to_owned(),
                    actual,
                    expected: expected.to_owned(),
                }
                .into())
            }
        }
        Ok(_) => Err(CheckoutViewError::UnmanagedOutputRoot(path.to_owned()).into()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error("inspect link", path, error)),
    }
}

/// Creates one directory symlink without selecting a different storage mode.
fn create_dir_symlink(target: &Path, link: &Path) -> io::Result<()> {
    #[cfg(unix)]
    return std::os::unix::fs::symlink(target, link);
    #[cfg(windows)]
    return std::os::windows::fs::symlink_dir(target, link);
}

/// Opens the sidecar lock that survives cleaning the managed view itself.
fn open_lock(path: &Path) -> bsmr_error::Result<File> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if !metadata.file_type().is_file() => {
            return Err(CheckoutViewError::UnmanagedEntry(path.to_owned()).into());
        }
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(io_error("inspect lock", path, error)),
    }
    OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(path)
        .map_err(|error| io_error("open lock", path, error))
}

/// Rejects a cache root redirected through a final-component symlink.
fn validate_directory(path: &Path) -> bsmr_error::Result<()> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| io_error("inspect managed directory", path, error))?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(CheckoutViewError::UnmanagedOutputRoot(path.to_owned()).into());
    }
    Ok(())
}

/// Maps one canonical checkout path to its mutable view identity.
fn view_key(project_root: &Path) -> String {
    let mut digest = Sha256::new();
    #[cfg(unix)]
    digest.update(project_root.as_os_str().as_bytes());
    #[cfg(windows)]
    for unit in project_root.as_os_str().encode_wide() {
        digest.update(unit.to_le_bytes());
    }
    hex::encode(digest.finalize())
}

/// Wraps one filesystem failure with the exact checkout-view operation.
fn io_error(
    operation: &'static str,
    path: impl AsRef<Path>,
    source: io::Error,
) -> bsmr_error::Error {
    CheckoutViewError::Io {
        operation,
        path: path.as_ref().to_owned(),
        source,
    }
    .into()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::sync::mpsc;
    use std::time::Duration;

    use bsmr_fs::paths::abs_norm_path::AbsNormPathBuf;

    use super::CheckoutViews;

    #[test]
    fn invariant_checkouts_receive_distinct_managed_views() -> bsmr_error::Result<()> {
        let temporary = tempfile::tempdir()?;
        let first = temporary.path().join("first");
        let second = temporary.path().join("second");
        fs::create_dir(&first)?;
        fs::create_dir(&second)?;
        let first = AbsNormPathBuf::new(first.canonicalize()?)?;
        let second = AbsNormPathBuf::new(second.canonicalize()?)?;
        let views = CheckoutViews::at(temporary.path().join("cache"))?;

        let _first_lease = views.prepare(&first)?;
        let _second_lease = views.prepare(&second)?;

        let first_view = fs::read_link(PathBuf::from(first.as_os_str()).join("bsmr-out"))?;
        let second_view = fs::read_link(PathBuf::from(second.as_os_str()).join("bsmr-out"))?;
        assert_ne!(first_view, second_view);
        assert!(first_view.is_dir());
        assert!(second_view.is_dir());
        Ok(())
    }

    #[test]
    fn invariant_same_checkout_daemons_share_the_view_lease() -> bsmr_error::Result<()> {
        let temporary = tempfile::tempdir()?;
        let project = temporary.path().join("project");
        fs::create_dir(&project)?;
        let project = Arc::new(AbsNormPathBuf::new(project.canonicalize()?)?);
        let views = Arc::new(CheckoutViews::at(temporary.path().join("cache"))?);
        let _first_lease = views.prepare(&project)?;
        let (sender, receiver) = mpsc::channel();
        std::thread::spawn({
            let project = project.clone();
            let views = views.clone();
            move || sender.send(views.prepare(&project).is_ok())
        });

        assert!(
            receiver
                .recv_timeout(Duration::from_secs(1))
                .expect("second daemon must acquire a shared lease")
        );
        Ok(())
    }

    #[test]
    fn design_unmanaged_output_root_is_rejected() -> bsmr_error::Result<()> {
        let temporary = tempfile::tempdir()?;
        let project = temporary.path().join("project");
        fs::create_dir(&project)?;
        fs::create_dir(project.join("bsmr-out"))?;
        let project = AbsNormPathBuf::new(project.canonicalize()?)?;
        let views = CheckoutViews::at(temporary.path().join("cache"))?;

        let error = views
            .prepare(&project)
            .expect_err("unmanaged output root must fail");

        assert!(error.to_string().contains("not a BSMR-managed link"));
        let managed_entries = temporary
            .path()
            .join("cache/v1")
            .read_dir()?
            .filter_map(Result::ok)
            .filter(|entry| {
                entry
                    .path()
                    .extension()
                    .is_none_or(|extension| extension != "lock")
            })
            .count();
        assert_eq!(managed_entries, 0);
        Ok(())
    }
}
