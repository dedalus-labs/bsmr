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
use bsmr_fs::paths::abs_path::AbsPath;
use serde::Deserialize;
use serde::Serialize;
use sha2::Digest;
use sha2::Sha256;

const OUTPUT_ROOT: &str = "bsmr-out";
const RECORD_VERSION: u8 = 1;
const VIEWS_VERSION: &str = "v1";

mod policy {
    pub(super) const GIB: u64 = 1024 * 1024 * 1024;
    pub(super) const MAX_BYTES: u64 = 100 * GIB;
    pub(super) const MAX_AGE_SECS: u64 = 30 * 24 * 60 * 60;
    pub(super) const MIN_BYTES: u64 = 10 * GIB;
    pub(super) const ROUND_BYTES: u64 = 5 * GIB;
}

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
    #[error("Checkout view record '{}' does not match its project root", _0.display())]
    RecordKeyMismatch(PathBuf),
    #[error("Checkout view record '{}' has unsupported version {version}", path.display())]
    RecordVersion { path: PathBuf, version: u8 },
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

/// Reports inactive checkout views reclaimed by one collection pass.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CheckoutViewCollection {
    pub removed_views: u64,
    pub removed_bytes: u64,
}

/// Bounds inactive checkout views by age and aggregate logical bytes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CheckoutViewPolicy {
    pub max_bytes: u64,
    pub max_age: std::time::Duration,
}

impl CheckoutViewPolicy {
    /// Resolves disk-scaled defaults and exact optional caller overrides.
    pub fn for_root(
        root: &Path,
        max_bytes: Option<u64>,
        max_age_secs: Option<u64>,
    ) -> bsmr_error::Result<Self> {
        let root = AbsPath::new(root)?;
        fs::create_dir_all(root).map_err(|error| io_error("create view root", root, error))?;
        validate_directory(root)?;
        let disk = bsmr_fs::fs_util::disk_space_stats(root)?;
        Ok(Self {
            max_bytes: max_bytes.unwrap_or_else(|| scaled_view_budget(disk.total_space)),
            max_age: std::time::Duration::from_secs(max_age_secs.unwrap_or(policy::MAX_AGE_SECS)),
        })
    }
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

    /// Collects missing, expired, and oldest over-budget inactive views.
    pub fn collect(
        &self,
        policy: CheckoutViewPolicy,
    ) -> bsmr_error::Result<CheckoutViewCollection> {
        let root = self.root.join(VIEWS_VERSION);
        validate_directory(&self.root)?;
        validate_directory(&root)?;
        let registry_lock = root.join("registry.lock");
        let registry = open_lock(&registry_lock)?;
        fs4::fs_std::FileExt::lock_exclusive(&registry)
            .map_err(|error| io_error("lock view registry", &registry_lock, error))?;

        let mut records = Vec::new();
        for entry in fs::read_dir(&root).map_err(|error| io_error("list", &root, error))? {
            let entry = entry.map_err(|error| io_error("list", &root, error))?;
            let record = entry.path();
            if record
                .extension()
                .is_none_or(|extension| extension != "json")
            {
                continue;
            }
            let value = read_record(&record)?;
            let key = view_key(&value.project_root);
            if record.file_stem().and_then(|stem| stem.to_str()) != Some(&key) {
                return Err(CheckoutViewError::RecordKeyMismatch(record).into());
            }
            let lock = root.join(format!("{key}.lock"));
            let file = open_existing_lock(&lock)?;
            let missing = match fs::metadata(&value.project_root) {
                Ok(_) => false,
                Err(error) if error.kind() == io::ErrorKind::NotFound => true,
                Err(error) => {
                    return Err(io_error(
                        "inspect recorded checkout",
                        &value.project_root,
                        error,
                    ));
                }
            };
            records.push((record, value, file, missing));
        }

        let mut candidates = Vec::new();
        for (record, value, file, missing) in records {
            match fs4::fs_std::FileExt::try_lock_exclusive(&file) {
                Ok(()) => {}
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => continue,
                Err(error) => return Err(io_error("lock view for collection", &record, error)),
            }
            let view = root.join(view_key(&value.project_root));
            let bytes = tree_bytes(&view)?;
            candidates.push((record, value, view, file, missing, bytes));
        }
        candidates.sort_by_key(|(record, value, ..)| (value.updated_secs, record.clone()));

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(CheckoutViewError::SystemTime)?
            .as_secs();
        let mut remaining_bytes = candidates
            .iter()
            .filter(|(_, _, _, _, missing, _)| !missing)
            .map(|(_, _, _, _, _, bytes)| bytes)
            .sum::<u64>();
        let mut outcome = CheckoutViewCollection::default();
        for (record, value, view, _, missing, bytes) in &candidates {
            let expired = now.saturating_sub(value.updated_secs) >= policy.max_age.as_secs();
            if !missing && !expired && remaining_bytes <= policy.max_bytes {
                continue;
            }
            remove_view(view)?;
            fs::remove_file(record).map_err(|error| io_error("remove record", record, error))?;
            outcome.removed_views += 1;
            outcome.removed_bytes += bytes;
            if !missing {
                remaining_bytes = remaining_bytes.saturating_sub(*bytes);
            }
        }

        drop(candidates);
        fs4::fs_std::FileExt::unlock(&registry)
            .map_err(|error| io_error("unlock view registry", &registry_lock, error))?;
        Ok(outcome)
    }
}

impl Drop for CheckoutViewLease {
    fn drop(&mut self) {
        if let Err(error) = fs4::fs_std::FileExt::unlock(&self.file) {
            tracing::warn!(%error, "checkout view lease release failed");
        }
    }
}

/// Writes the record while the registry excludes collection and the view lease is held.
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

/// Decodes one exact record before collection is authorized.
fn read_record(path: &Path) -> bsmr_error::Result<ViewRecord> {
    validate_regular_file(path)?;
    let contents = fs::read(path).map_err(|error| io_error("read record", path, error))?;
    let record: ViewRecord =
        serde_json::from_slice(&contents).map_err(|source| CheckoutViewError::DecodeRecord {
            path: path.to_owned(),
            source,
        })?;
    if record.version != RECORD_VERSION {
        return Err(CheckoutViewError::RecordVersion {
            path: path.to_owned(),
            version: record.version,
        }
        .into());
    }
    Ok(record)
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

/// Opens only a lock sidecar previously published by view preparation.
fn open_existing_lock(path: &Path) -> bsmr_error::Result<File> {
    validate_regular_file(path)?;
    OpenOptions::new()
        .read(true)
        .write(true)
        .open(path)
        .map_err(|error| io_error("open existing lock", path, error))
}

/// Rejects symlinks and other filesystem objects where a regular file is required.
fn validate_regular_file(path: &Path) -> bsmr_error::Result<()> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| io_error("inspect managed file", path, error))?;
    if !metadata.file_type().is_file() {
        return Err(CheckoutViewError::UnmanagedEntry(path.to_owned()).into());
    }
    Ok(())
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

/// Removes only an exact real view directory, never a symlink target.
fn remove_view(path: &Path) -> bsmr_error::Result<()> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(io_error("inspect view", path, error)),
    };
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(CheckoutViewError::UnmanagedOutputRoot(path.to_owned()).into());
    }
    fs::remove_dir_all(path).map_err(|error| io_error("remove view", path, error))
}

/// Counts regular file bytes without following symlinks outside the view.
fn tree_bytes(root: &Path) -> bsmr_error::Result<u64> {
    match fs::symlink_metadata(root) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
        Ok(_) => return Err(CheckoutViewError::UnmanagedOutputRoot(root.to_owned()).into()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(error) => return Err(io_error("inspect view", root, error)),
    }
    let mut bytes = 0;
    let mut pending = vec![root.to_owned()];
    while let Some(directory) = pending.pop() {
        for entry in
            fs::read_dir(&directory).map_err(|error| io_error("measure view", &directory, error))?
        {
            let entry = entry.map_err(|error| io_error("measure view", &directory, error))?;
            let file_type = entry
                .file_type()
                .map_err(|error| io_error("inspect view entry", entry.path(), error))?;
            if file_type.is_dir() {
                pending.push(entry.path());
            } else if file_type.is_file() {
                bytes += entry
                    .metadata()
                    .map_err(|error| io_error("measure view entry", entry.path(), error))?
                    .len();
            }
        }
    }
    Ok(bytes)
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

/// Scales the default inactive-view budget from ten percent of the backing disk.
fn scaled_view_budget(total_bytes: u64) -> u64 {
    let bounded = (total_bytes / 10).clamp(policy::MIN_BYTES, policy::MAX_BYTES);
    bounded / policy::ROUND_BYTES * policy::ROUND_BYTES
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

    use super::CheckoutViewPolicy;
    use super::CheckoutViews;
    use super::ViewRecord;
    use super::policy;
    use super::scaled_view_budget;
    use super::view_key;

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

    #[test]
    fn invariant_missing_checkout_view_is_collected() -> bsmr_error::Result<()> {
        let temporary = tempfile::tempdir()?;
        let project_path = temporary.path().join("project");
        fs::create_dir(&project_path)?;
        let project = AbsNormPathBuf::new(project_path.canonicalize()?)?;
        let views = CheckoutViews::at(temporary.path().join("cache"))?;
        let lease = views.prepare(&project)?;
        let view = fs::read_link(project_path.join("bsmr-out"))?;
        fs::write(view.join("artifact"), b"cached bytes")?;
        drop(lease);
        fs::remove_dir_all(&project_path)?;

        let collected = views.collect(CheckoutViewPolicy {
            max_bytes: u64::MAX,
            max_age: Duration::MAX,
        })?;

        assert_eq!(collected.removed_views, 1);
        assert_eq!(collected.removed_bytes, 12);
        assert!(!view.exists());
        Ok(())
    }

    #[test]
    fn invariant_active_view_is_never_collected() -> bsmr_error::Result<()> {
        let temporary = tempfile::tempdir()?;
        let project_path = temporary.path().join("project");
        fs::create_dir(&project_path)?;
        let project = AbsNormPathBuf::new(project_path.canonicalize()?)?;
        let views = CheckoutViews::at(temporary.path().join("cache"))?;
        let _lease = views.prepare(&project)?;
        let view = fs::read_link(project_path.join("bsmr-out"))?;
        fs::write(view.join("artifact"), b"active")?;

        let collected = views.collect(CheckoutViewPolicy {
            max_bytes: 0,
            max_age: Duration::ZERO,
        })?;

        assert_eq!(collected.removed_views, 0);
        assert!(view.exists());
        Ok(())
    }

    #[test]
    fn invariant_unowned_hash_directory_is_retained() -> bsmr_error::Result<()> {
        let temporary = tempfile::tempdir()?;
        let project = temporary.path().join("project");
        fs::create_dir(&project)?;
        let project = AbsNormPathBuf::new(project.canonicalize()?)?;
        let root = temporary.path().join("cache");
        let views = CheckoutViews::at(root.clone())?;
        let _lease = views.prepare(&project)?;
        let unowned = root.join("v1").join("a".repeat(64));
        fs::create_dir(&unowned)?;
        fs::write(unowned.join("artifact"), b"not ours")?;

        views.collect(CheckoutViewPolicy {
            max_bytes: 0,
            max_age: Duration::ZERO,
        })?;

        assert!(unowned.join("artifact").exists());
        Ok(())
    }

    #[test]
    fn design_invalid_record_blocks_collection_before_mutation() -> bsmr_error::Result<()> {
        let temporary = tempfile::tempdir()?;
        let missing_path = temporary.path().join("missing");
        let invalid_path = temporary.path().join("invalid");
        fs::create_dir(&missing_path)?;
        fs::create_dir(&invalid_path)?;
        let missing = AbsNormPathBuf::new(missing_path.canonicalize()?)?;
        let invalid = AbsNormPathBuf::new(invalid_path.canonicalize()?)?;
        let root = temporary.path().join("cache");
        let views = CheckoutViews::at(root.clone())?;
        let missing_lease = views.prepare(&missing)?;
        let invalid_lease = views.prepare(&invalid)?;
        let missing_view = fs::read_link(missing_path.join("bsmr-out"))?;
        drop(missing_lease);
        drop(invalid_lease);
        fs::remove_dir_all(&missing_path)?;
        let invalid_record = root
            .join("v1")
            .join(format!("{}.json", view_key(invalid.as_ref())));
        fs::write(invalid_record, b"not json")?;

        let error = views
            .collect(CheckoutViewPolicy {
                max_bytes: 0,
                max_age: Duration::ZERO,
            })
            .expect_err("invalid ownership metadata must stop collection");

        assert!(error.to_string().contains("decode checkout view record"));
        assert!(missing_view.exists());
        Ok(())
    }

    #[test]
    fn invariant_budget_evicts_oldest_inactive_view() -> bsmr_error::Result<()> {
        let temporary = tempfile::tempdir()?;
        let first_path = temporary.path().join("first");
        let second_path = temporary.path().join("second");
        fs::create_dir(&first_path)?;
        fs::create_dir(&second_path)?;
        let first = AbsNormPathBuf::new(first_path.canonicalize()?)?;
        let second = AbsNormPathBuf::new(second_path.canonicalize()?)?;
        let root = temporary.path().join("cache");
        let views = CheckoutViews::at(root.clone())?;
        let first_lease = views.prepare(&first)?;
        let second_lease = views.prepare(&second)?;
        let first_view = fs::read_link(first_path.join("bsmr-out"))?;
        let second_view = fs::read_link(second_path.join("bsmr-out"))?;
        fs::write(first_view.join("artifact"), b"first")?;
        fs::write(second_view.join("artifact"), b"second")?;
        drop(first_lease);
        drop(second_lease);
        let first_record = root
            .join("v1")
            .join(format!("{}.json", view_key(first.as_ref())));
        let mut record: ViewRecord = serde_json::from_slice(&fs::read(&first_record)?)?;
        record.updated_secs = 1;
        fs::write(&first_record, serde_json::to_vec(&record)?)?;

        let collected = views.collect(CheckoutViewPolicy {
            max_bytes: 6,
            max_age: Duration::MAX,
        })?;

        assert_eq!(collected.removed_views, 1);
        assert!(!first_view.exists());
        assert!(second_view.exists());
        Ok(())
    }

    #[test]
    fn view_budget_scales_within_fixed_bounds() {
        assert_eq!(scaled_view_budget(20 * policy::GIB), 10 * policy::GIB);
        assert_eq!(scaled_view_budget(500 * policy::GIB), 50 * policy::GIB);
        assert_eq!(scaled_view_budget(u64::MAX), 100 * policy::GIB);
    }
}
