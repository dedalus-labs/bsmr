//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Authoritative offline transport for a finalized local action cache.

use std::collections::BTreeMap;
use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::Read;
use std::io::Write;
use std::io::{self};
use std::path::Component;
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use prost::Message;
use serde::Deserialize;
use serde::Serialize;
use sha2::Digest;
use sha2::Sha256;

use super::LocalActionCache;
use super::LocalActionResult;
use super::LocalCacheError;
use super::LocalDigest;
use super::io_error;
use super::read_regular_file;
use crate::digest_config::DigestConfig;

const CACHE_SCHEMA: &str = "action-v1";
const MANIFEST: &str = "manifest.json";
const PACKAGE_SCHEMA: &str = "bsmr-local-cache-export-v1";
const PAYLOAD: &str = "payload";
static STAGING_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, bsmr_error::Error)]
#[bsmr(tag = Environment)]
enum TransportError {
    #[error("Cache transport output already exists: '{}'", _0.display())]
    Exists(PathBuf),
    #[error("Cache transport engine mismatch: archive {archive}, current {current}")]
    Engine { archive: String, current: String },
    #[error("Cache transport manifest is invalid: {0}")]
    Manifest(String),
    #[error("Cache transport package has an unexpected entry: '{}'", _0.display())]
    Unexpected(PathBuf),
    #[error("Cache transport path must be absolute: '{}'", _0.display())]
    Relative(PathBuf),
    #[error("Cache transport is unsupported on this platform")]
    UnsupportedPlatform,
    #[error("Cache transport activated '{}' but could not sync its parent: {source}", path.display())]
    ActivatedDurability {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("System clock is earlier than the Unix epoch: {0}")]
    SystemTime(#[source] std::time::SystemTimeError),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LocalCacheExportManifest {
    pub schema: String,
    pub cache_schema: String,
    pub engine_sha256: String,
    pub digest_config: String,
    pub created_secs: u64,
    pub files: Vec<LocalCacheExportFile>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LocalCacheExportFile {
    pub path: String,
    pub size: u64,
    pub sha256: String,
    pub modified_secs: u64,
    pub modified_nanos: u32,
}

impl LocalActionCache {
    /// Exports one complete cache into a new atomically published package directory.
    pub fn export_package(
        &self,
        output: &Path,
        engine_sha256: &str,
        digest_config: DigestConfig,
    ) -> bsmr_error::Result<LocalCacheExportManifest> {
        require_supported()?;
        if !is_hex(engine_sha256, 64) {
            return Err(TransportError::Manifest("engine SHA-256 is invalid".to_owned()).into());
        }
        require_absent(output)?;
        let _lock = self.exclusive_lock()?;
        validate_actions(self, digest_config)?;
        let staging = create_staging(output, "export")?;
        let payload = staging.join(PAYLOAD);
        fs::create_dir(&payload)
            .map_err(|error| io_error("create export payload", &payload, error))?;
        let result = (|| {
            let mut files = Vec::new();
            for area in ["ac", "cas"] {
                for source in durable_files(&self.root.join(area), true)? {
                    let relative = source
                        .strip_prefix(&self.root)
                        .expect("cache entries are below their root");
                    let destination = payload.join(relative);
                    copy_file(&source, &destination)?;
                    files.push(export_file(&destination, &staging)?);
                }
            }
            files.sort_by(|left, right| left.path.cmp(&right.path));
            let manifest = LocalCacheExportManifest {
                schema: PACKAGE_SCHEMA.to_owned(),
                cache_schema: CACHE_SCHEMA.to_owned(),
                engine_sha256: engine_sha256.to_owned(),
                digest_config: digest_config.to_string(),
                created_secs: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map_err(TransportError::SystemTime)?
                    .as_secs(),
                files,
            };
            write_manifest(&staging.join(MANIFEST), &manifest)?;
            sync_tree(&staging)?;
            publish_directory(&staging, output)?;
            sync_parent(output)?;
            Ok(manifest)
        })();
        if result.is_err() {
            let _ignored = remove_entry(&staging);
        }
        result
    }

    /// Imports one verified package into an absent cache root and activates it atomically.
    pub fn import_package(
        &self,
        input: &Path,
        engine_sha256: &str,
        digest_config: DigestConfig,
    ) -> bsmr_error::Result<LocalCacheExportManifest> {
        require_supported()?;
        require_absolute(input)?;
        require_absent(&self.root)?;
        let manifest = read_manifest(&input.join(MANIFEST))?;
        validate_manifest(&manifest, engine_sha256, digest_config)?;
        let package_files = package_files(input)?;
        let declared = manifest
            .files
            .iter()
            .cloned()
            .map(|file| (file.path.clone(), file))
            .collect::<BTreeMap<_, _>>();
        if package_files.len() != declared.len() {
            return Err(TransportError::Manifest("package file count differs".to_owned()).into());
        }
        let staging = create_staging(&self.root, "import")?;
        let result = (|| {
            for (path, source) in package_files {
                let expected = declared
                    .get(&path)
                    .ok_or_else(|| TransportError::Unexpected(source.clone()))?;
                let destination = staging.join(cache_relative(&path)?);
                import_file(&source, &destination, expected)?;
                validate_file(&destination, expected)?;
            }
            validate_actions(&LocalActionCache::at(staging.clone())?, digest_config)?;
            sync_tree(&staging)?;
            publish_directory(&staging, &self.root)?;
            sync_parent(&self.root)?;
            Ok(manifest)
        })();
        if result.is_err() {
            let _ignored = remove_entry(&staging);
        }
        result
    }
}

pub fn sha256_file(path: &Path) -> bsmr_error::Result<String> {
    let mut input = File::open(path).map_err(|error| io_error("open for sha256", path, error))?;
    let mut digest = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let length = input
            .read(&mut buffer)
            .map_err(|error| io_error("read for sha256", path, error))?;
        if length == 0 {
            break;
        }
        digest.update(&buffer[..length]);
    }
    Ok(hex::encode(digest.finalize()))
}

fn validate_actions(
    cache: &LocalActionCache,
    digest_config: DigestConfig,
) -> bsmr_error::Result<()> {
    let inventory = cache.inventory_unlocked(digest_config)?;
    if inventory.incomplete_actions != 0 {
        return Err(TransportError::Manifest(format!(
            "{} incomplete actions",
            inventory.incomplete_actions
        ))
        .into());
    }
    for action in durable_files(&cache.root.join("ac"), true)? {
        let bytes = read_regular_file(&action)?.expect("finalized action exists");
        let result: LocalActionResult =
            serde_json::from_slice(&bytes).map_err(|source| LocalCacheError::DecodeAction {
                path: action,
                source,
            })?;
        validate_result(cache, &result, digest_config)?;
    }
    Ok(())
}

fn validate_result(
    cache: &LocalActionCache,
    result: &LocalActionResult,
    digest_config: DigestConfig,
) -> bsmr_error::Result<()> {
    for digest in result
        .output_files
        .iter()
        .map(|file| &file.digest)
        .chain(result.stdout.iter())
        .chain(result.stderr.iter())
    {
        if cache.read_blob_unlocked(digest, digest_config)?.is_none() {
            return Err(
                TransportError::Manifest("action references a missing blob".to_owned()).into(),
            );
        }
    }
    for directory in &result.output_directories {
        let digest = &directory.tree_digest;
        let Some(bytes) = cache.read_blob_unlocked(digest, digest_config)? else {
            return Err(
                TransportError::Manifest("action references a missing tree".to_owned()).into(),
            );
        };
        let tree = remote_execution::Tree::decode(bytes.as_slice()).map_err(|source| {
            LocalCacheError::DecodeTree {
                path: cache.blob_path(digest),
                source,
            }
        })?;
        let root = tree.root.as_ref().ok_or(LocalCacheError::MissingTreeRoot)?;
        for file in std::iter::once(root)
            .chain(tree.children.iter())
            .flat_map(|directory| &directory.files)
        {
            let file = file
                .digest
                .as_ref()
                .ok_or(LocalCacheError::MissingFileDigest)?;
            let file = LocalDigest {
                algorithm: digest.algorithm.clone(),
                hash: file.hash.clone(),
                size: file.size_bytes,
            };
            if cache.read_blob_unlocked(&file, digest_config)?.is_none() {
                return Err(
                    TransportError::Manifest("tree references a missing blob".to_owned()).into(),
                );
            }
        }
    }
    Ok(())
}

fn durable_files(root: &Path, ignore_temporary: bool) -> bsmr_error::Result<Vec<PathBuf>> {
    let prefixes = match fs::read_dir(root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(io_error("list cache transport area", root, error)),
    };
    let mut files = Vec::new();
    for prefix in prefixes {
        let prefix = prefix.map_err(|error| io_error("list cache transport area", root, error))?;
        let name = prefix.file_name();
        let name = name
            .to_str()
            .ok_or_else(|| TransportError::Unexpected(prefix.path()))?;
        if !is_hex(name, 2)
            || !prefix
                .file_type()
                .map_err(|error| io_error("inspect cache prefix", prefix.path(), error))?
                .is_dir()
        {
            return Err(TransportError::Unexpected(prefix.path()).into());
        }
        for file in fs::read_dir(prefix.path())
            .map_err(|error| io_error("list cache prefix", prefix.path(), error))?
        {
            let file = file.map_err(|error| io_error("list cache prefix", prefix.path(), error))?;
            let path = file.path();
            if ignore_temporary && super::inventory::is_temporary_key(&path) {
                continue;
            }
            let key = file.file_name();
            let key = key
                .to_str()
                .ok_or_else(|| TransportError::Unexpected(path.clone()))?;
            if !is_hex(key, 64)
                || !key.starts_with(name)
                || !file
                    .file_type()
                    .map_err(|error| io_error("inspect cache entry", &path, error))?
                    .is_file()
            {
                return Err(TransportError::Unexpected(path).into());
            }
            files.push(path);
        }
    }
    files.sort();
    Ok(files)
}

fn package_files(root: &Path) -> bsmr_error::Result<BTreeMap<String, PathBuf>> {
    let root_type = fs::symlink_metadata(root)
        .map_err(|error| io_error("inspect cache package", root, error))?
        .file_type();
    if !root_type.is_dir() {
        return Err(TransportError::Unexpected(root.to_owned()).into());
    }
    let mut top = fs::read_dir(root)
        .map_err(|error| io_error("list cache package", root, error))?
        .map(|entry| entry.map(|entry| entry.file_name()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| io_error("list cache package", root, error))?;
    top.sort();
    if top
        != vec![
            std::ffi::OsString::from(MANIFEST),
            std::ffi::OsString::from(PAYLOAD),
        ]
    {
        return Err(TransportError::Manifest(
            "package root must contain only manifest.json and payload".to_owned(),
        )
        .into());
    }
    let payload = root.join(PAYLOAD);
    let payload_type = fs::symlink_metadata(&payload)
        .map_err(|error| io_error("inspect cache package payload", &payload, error))?
        .file_type();
    if !payload_type.is_dir() {
        return Err(TransportError::Unexpected(payload).into());
    }
    for entry in fs::read_dir(&payload)
        .map_err(|error| io_error("list cache package payload", &payload, error))?
    {
        let entry =
            entry.map_err(|error| io_error("list cache package payload", &payload, error))?;
        let name = entry.file_name();
        if (name != "ac" && name != "cas")
            || !entry
                .file_type()
                .map_err(|error| io_error("inspect cache package area", entry.path(), error))?
                .is_dir()
        {
            return Err(TransportError::Unexpected(entry.path()).into());
        }
    }
    let mut files = BTreeMap::new();
    for area in ["ac", "cas"] {
        for path in durable_files(&root.join(PAYLOAD).join(area), false)? {
            let relative = path
                .strip_prefix(root)
                .expect("package files are below root");
            let relative = slash_path(relative)?;
            if files.insert(relative, path.clone()).is_some() {
                return Err(TransportError::Unexpected(path).into());
            }
        }
    }
    Ok(files)
}

fn validate_manifest(
    manifest: &LocalCacheExportManifest,
    engine: &str,
    digest: DigestConfig,
) -> bsmr_error::Result<()> {
    if manifest.schema != PACKAGE_SCHEMA || manifest.cache_schema != CACHE_SCHEMA {
        return Err(TransportError::Manifest("unsupported schema".to_owned()).into());
    }
    if !is_hex(&manifest.engine_sha256, 64) || manifest.engine_sha256 != engine {
        return Err(TransportError::Engine {
            archive: manifest.engine_sha256.clone(),
            current: engine.to_owned(),
        }
        .into());
    }
    if manifest.digest_config != digest.to_string() {
        return Err(TransportError::Manifest("digest policy mismatch".to_owned()).into());
    }
    let mut previous = None;
    for file in &manifest.files {
        cache_relative(&file.path)?;
        if !is_hex(&file.sha256, 64) || file.modified_nanos >= 1_000_000_000 {
            return Err(
                TransportError::Manifest(format!("invalid file record: {}", file.path)).into(),
            );
        }
        if previous.as_ref().is_some_and(|path| path >= &file.path) {
            return Err(
                TransportError::Manifest("file paths are not strictly sorted".to_owned()).into(),
            );
        }
        previous = Some(file.path.clone());
    }
    Ok(())
}

fn cache_relative(path: &str) -> bsmr_error::Result<PathBuf> {
    let parts = path.split('/').collect::<Vec<_>>();
    let valid = matches!(parts.as_slice(), [payload, area, prefix, key]
        if *payload == PAYLOAD && (*area == "ac" || *area == "cas")
            && is_hex(prefix, 2) && is_hex(key, 64) && key.starts_with(prefix));
    if !valid {
        return Err(TransportError::Manifest(format!("invalid payload path: {path}")).into());
    }
    Ok(PathBuf::from(parts[1]).join(parts[2]).join(parts[3]))
}

fn is_hex(value: &str, length: usize) -> bool {
    value.len() == length
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn export_file(path: &Path, root: &Path) -> bsmr_error::Result<LocalCacheExportFile> {
    let metadata =
        fs::metadata(path).map_err(|error| io_error("inspect exported file", path, error))?;
    let modified = metadata
        .modified()
        .map_err(|error| io_error("read exported mtime", path, error))?
        .duration_since(UNIX_EPOCH)
        .map_err(TransportError::SystemTime)?;
    Ok(LocalCacheExportFile {
        path: slash_path(
            path.strip_prefix(root)
                .expect("exported files are below root"),
        )?,
        size: metadata.len(),
        sha256: sha256_file(path)?,
        modified_secs: modified.as_secs(),
        modified_nanos: modified.subsec_nanos(),
    })
}

fn validate_file(path: &Path, expected: &LocalCacheExportFile) -> bsmr_error::Result<()> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|error| io_error("inspect package file", path, error))?;
    if !metadata.file_type().is_file()
        || metadata.len() != expected.size
        || sha256_file(path)? != expected.sha256
    {
        return Err(
            TransportError::Manifest(format!("payload mismatch: {}", expected.path)).into(),
        );
    }
    Ok(())
}

fn copy_file(source: &Path, destination: &Path) -> bsmr_error::Result<()> {
    let metadata = fs::symlink_metadata(source)
        .map_err(|error| io_error("inspect transport source", source, error))?;
    if !metadata.file_type().is_file() {
        return Err(TransportError::Unexpected(source.to_owned()).into());
    }
    let parent = destination.parent().expect("transport files have a parent");
    fs::create_dir_all(parent)
        .map_err(|error| io_error("create transport directory", parent, error))?;
    fs::copy(source, destination)
        .map_err(|error| io_error("copy transport file", destination, error))?;
    let output = OpenOptions::new()
        .write(true)
        .open(destination)
        .map_err(|error| io_error("open copied file", destination, error))?;
    output
        .set_times(
            fs::FileTimes::new().set_modified(
                metadata
                    .modified()
                    .map_err(|error| io_error("read source mtime", source, error))?,
            ),
        )
        .map_err(|error| io_error("set copied mtime", destination, error))?;
    output
        .sync_all()
        .map_err(|error| io_error("sync copied file", destination, error))
}

fn import_file(
    source: &Path,
    destination: &Path,
    expected: &LocalCacheExportFile,
) -> bsmr_error::Result<()> {
    let mut input = open_package_file(source)?;
    if input
        .metadata()
        .map_err(|error| io_error("inspect package file", source, error))?
        .len()
        != expected.size
    {
        return Err(
            TransportError::Manifest(format!("payload size mismatch: {}", expected.path)).into(),
        );
    }
    let parent = destination.parent().expect("imported files have a parent");
    fs::create_dir_all(parent)
        .map_err(|error| io_error("create import directory", parent, error))?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|error| io_error("create imported cache file", destination, error))?;
    let mut sha256 = Sha256::new();
    let mut size = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let length = input
            .read(&mut buffer)
            .map_err(|error| io_error("read package file", source, error))?;
        if length == 0 {
            break;
        }
        output
            .write_all(&buffer[..length])
            .map_err(|error| io_error("write imported cache file", destination, error))?;
        sha256.update(&buffer[..length]);
        size += length as u64;
    }
    if size != expected.size || hex::encode(sha256.finalize()) != expected.sha256 {
        return Err(
            TransportError::Manifest(format!("payload mismatch: {}", expected.path)).into(),
        );
    }
    let modified = UNIX_EPOCH
        .checked_add(std::time::Duration::new(
            expected.modified_secs,
            expected.modified_nanos,
        ))
        .ok_or_else(|| TransportError::Manifest("file timestamp is out of range".to_owned()))?;
    output
        .set_times(fs::FileTimes::new().set_modified(modified))
        .map_err(|error| io_error("restore cache mtime", destination, error))?;
    output
        .sync_all()
        .map_err(|error| io_error("sync imported cache file", destination, error))
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn open_package_file(path: &Path) -> bsmr_error::Result<File> {
    use std::os::unix::fs::MetadataExt;

    use rustix::fs::Mode;
    use rustix::fs::OFlags;

    let fd = rustix::fs::open(
        path,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .map_err(|error| io_error("open package file", path, error.into()))?;
    let file = File::from(fd);
    let metadata = file
        .metadata()
        .map_err(|error| io_error("inspect package file", path, error))?;
    if !metadata.file_type().is_file() || metadata.nlink() != 1 {
        return Err(TransportError::Unexpected(path.to_owned()).into());
    }
    Ok(file)
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn open_package_file(_path: &Path) -> bsmr_error::Result<File> {
    Err(TransportError::UnsupportedPlatform.into())
}

fn write_manifest(path: &Path, manifest: &LocalCacheExportManifest) -> bsmr_error::Result<()> {
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| io_error("create transport manifest", path, error))?;
    serde_json::to_writer_pretty(&mut output, manifest)
        .map_err(|error| TransportError::Manifest(error.to_string()))?;
    output
        .write_all(b"\n")
        .map_err(|error| io_error("write transport manifest", path, error))?;
    output
        .sync_all()
        .map_err(|error| io_error("sync transport manifest", path, error))
}

fn read_manifest(path: &Path) -> bsmr_error::Result<LocalCacheExportManifest> {
    let mut input = open_package_file(path)?;
    let mut bytes = Vec::new();
    input
        .read_to_end(&mut bytes)
        .map_err(|error| io_error("read transport manifest", path, error))?;
    serde_json::from_slice(&bytes)
        .map_err(|error| TransportError::Manifest(error.to_string()).into())
}

fn slash_path(path: &Path) -> bsmr_error::Result<String> {
    let parts = path
        .components()
        .map(|part| match part {
            Component::Normal(value) => value.to_str(),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| TransportError::Unexpected(path.to_owned()))?;
    Ok(parts.join("/"))
}

fn require_absent(path: &Path) -> bsmr_error::Result<()> {
    require_absolute(path)?;
    match fs::symlink_metadata(path) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error("inspect transport destination", path, error)),
        Ok(_) => Err(TransportError::Exists(path.to_owned()).into()),
    }
}

fn require_absolute(path: &Path) -> bsmr_error::Result<()> {
    if path.is_absolute() {
        Ok(())
    } else {
        Err(TransportError::Relative(path.to_owned()).into())
    }
}

fn require_supported() -> bsmr_error::Result<()> {
    if cfg!(any(target_os = "linux", target_os = "macos")) {
        Ok(())
    } else {
        Err(TransportError::UnsupportedPlatform.into())
    }
}

fn create_staging(path: &Path, kind: &str) -> bsmr_error::Result<PathBuf> {
    let parent = path.parent().expect("transport destinations have a parent");
    loop {
        let id = STAGING_ID.fetch_add(1, Ordering::Relaxed);
        let staging = parent.join(format!(".bsmr-cache-{kind}.{}.{id}", std::process::id()));
        match fs::create_dir(&staging) {
            Ok(()) => return Ok(staging),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(io_error("create transport staging", staging, error)),
        }
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn publish_directory(staging: &Path, destination: &Path) -> bsmr_error::Result<()> {
    use rustix::fs::CWD;
    use rustix::fs::RenameFlags;

    rustix::fs::renameat_with(CWD, staging, CWD, destination, RenameFlags::NOREPLACE).map_err(
        |error| {
            let error: io::Error = error.into();
            if error.kind() == io::ErrorKind::AlreadyExists {
                TransportError::Exists(destination.to_owned()).into()
            } else {
                io_error("publish cache transport", destination, error)
            }
        },
    )
}

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
fn publish_directory(_staging: &Path, _destination: &Path) -> bsmr_error::Result<()> {
    Err(TransportError::UnsupportedPlatform.into())
}

fn sync_tree(root: &Path) -> bsmr_error::Result<()> {
    let mut directories = vec![root.to_owned()];
    let mut index = 0;
    while index < directories.len() {
        for entry in fs::read_dir(&directories[index])
            .map_err(|error| io_error("list transport tree", &directories[index], error))?
        {
            let entry = entry
                .map_err(|error| io_error("list transport tree", &directories[index], error))?;
            if entry
                .file_type()
                .map_err(|error| io_error("inspect transport tree", entry.path(), error))?
                .is_dir()
            {
                directories.push(entry.path());
            }
        }
        index += 1;
    }
    for directory in directories.into_iter().rev() {
        File::open(&directory)
            .and_then(|file| file.sync_all())
            .map_err(|error| io_error("sync transport directory", directory, error))?;
    }
    Ok(())
}

fn sync_parent(path: &Path) -> bsmr_error::Result<()> {
    let parent = path.parent().expect("transport paths have a parent");
    File::open(parent)
        .and_then(|file| file.sync_all())
        .map_err(|source| {
            TransportError::ActivatedDurability {
                path: path.to_owned(),
                source,
            }
            .into()
        })
}

fn remove_entry(path: &Path) -> io::Result<()> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_dir() => {
            fs::remove_file(path)
        }
        Ok(_) => fs::remove_dir_all(path),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;

    use bsmr_common::cas_digest::DigestAlgorithm;
    use bsmr_common::file_ops::metadata::TrackedFileDigest;

    use super::super::LocalOutputFile;
    use super::LocalActionCache;
    use super::LocalActionResult;
    use super::LocalDigest;
    use crate::digest_config::DigestConfig;
    use crate::execute::action_digest::ActionDigest;

    fn exported_fixture() -> bsmr_error::Result<(
        tempfile::TempDir,
        PathBuf,
        LocalActionCache,
        DigestConfig,
        String,
    )> {
        let temporary = tempfile::tempdir()?;
        let source = LocalActionCache::at(temporary.path().join("source"))?;
        let package = temporary.path().join("export");
        let destination = LocalActionCache::at(temporary.path().join("destination"))?;
        let digest_config = DigestConfig::testing_default();
        let action = ActionDigest::from_content(b"action", digest_config.cas_digest_config());
        let output =
            TrackedFileDigest::from_content(b"cached output", digest_config.cas_digest_config());
        let result = LocalActionResult {
            output_files: vec![LocalOutputFile {
                path: "bsmr-out/output".to_owned(),
                digest: LocalDigest::from_file(&output),
                executable: false,
            }],
            ..Default::default()
        };
        source.publish_bytes(&output, b"cached output", digest_config)?;
        source.publish_action_result(&action, &result)?;
        let engine = "a".repeat(64);
        source.export_package(&package, &engine, digest_config)?;
        Ok((temporary, package, destination, digest_config, engine))
    }

    fn manifest(package: &Path) -> bsmr_error::Result<super::LocalCacheExportManifest> {
        Ok(serde_json::from_slice(&fs::read(
            package.join(super::MANIFEST),
        )?)?)
    }

    fn assert_import_rejected(
        package: &Path,
        destination: &LocalActionCache,
        engine: &str,
        digest_config: DigestConfig,
    ) {
        destination
            .import_package(package, engine, digest_config)
            .expect_err("invalid package must be rejected");
        assert!(!destination.root.exists());
    }

    #[test]
    fn invariant_export_contains_only_complete_durable_cache_state() -> bsmr_error::Result<()> {
        let temporary = tempfile::tempdir()?;
        let root = temporary.path().join("cache");
        let package = temporary.path().join("export");
        let cache = LocalActionCache::at(root.clone())?;
        let digest_config = DigestConfig::testing_default();
        let action = ActionDigest::from_content(b"action", digest_config.cas_digest_config());
        let output =
            TrackedFileDigest::from_content(b"cached output", digest_config.cas_digest_config());
        let result = LocalActionResult {
            output_files: vec![LocalOutputFile {
                path: "bsmr-out/output".to_owned(),
                digest: LocalDigest::from_file(&output),
                executable: false,
            }],
            ..Default::default()
        };
        cache.publish_bytes(&output, b"cached output", digest_config)?;
        cache.publish_action_result(&action, &result)?;
        let interrupted = cache
            .blob_path(&LocalDigest::from_file(&output))
            .with_extension("tmp.123.0");
        fs::write(&interrupted, b"interrupted")?;
        fs::create_dir_all(root.join("flights"))?;
        fs::write(root.join("flights/receipt"), b"operational")?;

        let manifest = cache.export_package(&package, &"a".repeat(64), digest_config)?;

        assert_eq!(manifest.files.len(), 2);
        assert!(manifest.files[0].path < manifest.files[1].path);
        assert!(manifest.files.iter().all(|file| {
            (file.path.starts_with("payload/ac/") || file.path.starts_with("payload/cas/"))
                && !file.path.contains(".tmp.")
        }));
        assert_eq!(
            serde_json::from_slice::<super::LocalCacheExportManifest>(&fs::read(
                package.join(super::MANIFEST)
            )?)?,
            manifest
        );
        Ok(())
    }

    #[test]
    fn invariant_import_recreates_the_verified_hit_in_an_absent_root() -> bsmr_error::Result<()> {
        let temporary = tempfile::tempdir()?;
        let source = LocalActionCache::at(temporary.path().join("source"))?;
        let package = temporary.path().join("export");
        let destination = LocalActionCache::at(temporary.path().join("destination"))?;
        let digest_config = DigestConfig::testing_default();
        let action = ActionDigest::from_content(b"action", digest_config.cas_digest_config());
        let output =
            TrackedFileDigest::from_content(b"cached output", digest_config.cas_digest_config());
        let result = LocalActionResult {
            output_files: vec![LocalOutputFile {
                path: "bsmr-out/output".to_owned(),
                digest: LocalDigest::from_file(&output),
                executable: false,
            }],
            ..Default::default()
        };
        source.publish_bytes(&output, b"cached output", digest_config)?;
        source.publish_action_result(&action, &result)?;
        let engine = "a".repeat(64);
        let exported = source.export_package(&package, &engine, digest_config)?;

        let imported = destination.import_package(&package, &engine, digest_config)?;

        assert_eq!(imported, exported);
        assert_eq!(destination.action_result(&action)?, Some(result));
        assert_eq!(
            destination.read_blob(&LocalDigest::from_file(&output), digest_config)?,
            Some(b"cached output".to_vec())
        );
        let root_entries = fs::read_dir(temporary.path().join("destination"))?
            .map(|entry| entry.map(|entry| entry.file_name()))
            .collect::<Result<Vec<_>, _>>()?;
        assert!(
            root_entries
                .iter()
                .all(|entry| entry == "ac" || entry == "cas" || entry == "cache.lock")
        );
        Ok(())
    }

    #[test]
    fn invariant_failed_retry_keeps_successful_prerequisite_only() -> bsmr_error::Result<()> {
        let temporary = tempfile::tempdir()?;
        let source = LocalActionCache::at(temporary.path().join("source"))?;
        let destination = LocalActionCache::at(temporary.path().join("destination"))?;
        let package = temporary.path().join("package");
        let digest_config = DigestConfig::testing_default();
        let prerequisite =
            ActionDigest::from_content(b"prerequisite", digest_config.cas_digest_config());
        let failed = ActionDigest::from_content(b"failed", digest_config.cas_digest_config());
        let output = TrackedFileDigest::from_content(
            b"prerequisite output",
            digest_config.cas_digest_config(),
        );
        let result = LocalActionResult {
            output_files: vec![LocalOutputFile {
                path: "bsmr-out/prerequisite".to_owned(),
                digest: LocalDigest::from_file(&output),
                executable: false,
            }],
            ..Default::default()
        };
        source.publish_bytes(&output, b"prerequisite output", digest_config)?;
        source.publish_action_result(&prerequisite, &result)?;
        let engine = "a".repeat(64);
        source.export_package(&package, &engine, digest_config)?;

        destination.import_package(&package, &engine, digest_config)?;

        assert_eq!(destination.action_result(&prerequisite)?, Some(result));
        assert_eq!(destination.action_result(&failed)?, None);
        Ok(())
    }

    #[test]
    fn import_rejects_same_size_payload_mutation() -> bsmr_error::Result<()> {
        let (_temporary, package, destination, digest_config, engine) = exported_fixture()?;
        let payload = package.join(&manifest(&package)?.files[0].path);
        let mut bytes = fs::read(&payload)?;
        bytes[0] ^= 1;
        fs::write(payload, bytes)?;

        assert_import_rejected(&package, &destination, &engine, digest_config);
        Ok(())
    }

    #[test]
    fn import_rejects_truncated_payload() -> bsmr_error::Result<()> {
        let (_temporary, package, destination, digest_config, engine) = exported_fixture()?;
        let payload = package.join(&manifest(&package)?.files[0].path);
        let file = fs::OpenOptions::new().write(true).open(payload)?;
        file.set_len(file.metadata()?.len() - 1)?;

        assert_import_rejected(&package, &destination, &engine, digest_config);
        Ok(())
    }

    #[test]
    fn import_rejects_incomplete_package() -> bsmr_error::Result<()> {
        let (_temporary, package, destination, digest_config, engine) = exported_fixture()?;
        let payload = package.join(&manifest(&package)?.files[0].path);
        fs::remove_file(payload)?;

        assert_import_rejected(&package, &destination, &engine, digest_config);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn import_rejects_payload_symlink() -> bsmr_error::Result<()> {
        let (_temporary, package, destination, digest_config, engine) = exported_fixture()?;
        let payload = package.join(&manifest(&package)?.files[0].path);
        fs::remove_file(&payload)?;
        std::os::unix::fs::symlink(package.join(super::MANIFEST), payload)?;

        assert_import_rejected(&package, &destination, &engine, digest_config);
        Ok(())
    }

    #[test]
    fn import_rejects_traversal_manifest_path() -> bsmr_error::Result<()> {
        let (_temporary, package, destination, digest_config, engine) = exported_fixture()?;
        let mut manifest = manifest(&package)?;
        manifest.files[0].path = "payload/ac/../../escape".to_owned();
        fs::write(
            package.join(super::MANIFEST),
            serde_json::to_vec_pretty(&manifest)?,
        )?;

        assert_import_rejected(&package, &destination, &engine, digest_config);
        Ok(())
    }

    #[test]
    fn import_rejects_mismatched_engine() -> bsmr_error::Result<()> {
        let (_temporary, package, destination, digest_config, _engine) = exported_fixture()?;

        assert_import_rejected(&package, &destination, &"b".repeat(64), digest_config);
        Ok(())
    }

    #[test]
    fn import_rejects_operational_temporary_file() -> bsmr_error::Result<()> {
        let (_temporary, package, destination, digest_config, engine) = exported_fixture()?;
        let payload = package.join(&manifest(&package)?.files[0].path);
        fs::write(payload.with_extension("tmp.123.0"), b"temporary")?;

        assert_import_rejected(&package, &destination, &engine, digest_config);
        Ok(())
    }

    #[test]
    fn atomic_activation_never_replaces_an_existing_destination() -> bsmr_error::Result<()> {
        let temporary = tempfile::tempdir()?;
        let staging = temporary.path().join("staging");
        let destination = temporary.path().join("destination");
        fs::create_dir(&staging)?;
        fs::write(staging.join("new"), b"new")?;
        fs::create_dir(&destination)?;
        fs::write(destination.join("existing"), b"existing")?;

        super::publish_directory(&staging, &destination)
            .expect_err("activation must not replace an existing directory");

        assert_eq!(fs::read(destination.join("existing"))?, b"existing");
        assert_eq!(fs::read(staging.join("new"))?, b"new");
        Ok(())
    }

    #[test]
    fn competing_activations_publish_exactly_one_directory() -> bsmr_error::Result<()> {
        let temporary = tempfile::tempdir()?;
        let first = temporary.path().join("first");
        let second = temporary.path().join("second");
        let destination = temporary.path().join("destination");
        fs::create_dir(&first)?;
        fs::create_dir(&second)?;
        fs::write(first.join("winner"), b"first")?;
        fs::write(second.join("winner"), b"second")?;
        let barrier = std::sync::Barrier::new(3);

        let outcomes = std::thread::scope(|scope| {
            let first = scope.spawn(|| {
                barrier.wait();
                super::publish_directory(&first, &destination)
            });
            let second = scope.spawn(|| {
                barrier.wait();
                super::publish_directory(&second, &destination)
            });
            barrier.wait();
            [
                first.join().expect("first activation"),
                second.join().expect("second activation"),
            ]
        });

        assert_eq!(outcomes.iter().filter(|outcome| outcome.is_ok()).count(), 1);
        assert!(matches!(
            fs::read(destination.join("winner"))?.as_slice(),
            b"first" | b"second"
        ));
        Ok(())
    }

    #[test]
    fn import_rejects_wrong_digest_policy() -> bsmr_error::Result<()> {
        let (_temporary, package, destination, _digest_config, engine) = exported_fixture()?;
        let sha256 = DigestConfig::leak_new(vec![DigestAlgorithm::Sha256], None)?;

        assert_import_rejected(&package, &destination, &engine, sha256);
        Ok(())
    }

    #[test]
    fn import_rejects_unsupported_schema() -> bsmr_error::Result<()> {
        let (_temporary, package, destination, digest_config, engine) = exported_fixture()?;
        let mut manifest = manifest(&package)?;
        manifest.schema = "future".to_owned();
        fs::write(
            package.join(super::MANIFEST),
            serde_json::to_vec_pretty(&manifest)?,
        )?;

        assert_import_rejected(&package, &destination, &engine, digest_config);
        Ok(())
    }

    #[test]
    fn import_rejects_timestamp_overflow() -> bsmr_error::Result<()> {
        let (_temporary, package, destination, digest_config, engine) = exported_fixture()?;
        let mut manifest = manifest(&package)?;
        manifest.files[0].modified_secs = u64::MAX;
        fs::write(
            package.join(super::MANIFEST),
            serde_json::to_vec_pretty(&manifest)?,
        )?;

        assert_import_rejected(&package, &destination, &engine, digest_config);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn import_rejects_hard_linked_payload() -> bsmr_error::Result<()> {
        let (temporary, package, destination, digest_config, engine) = exported_fixture()?;
        let payload = package.join(&manifest(&package)?.files[0].path);
        fs::hard_link(&payload, temporary.path().join("outside-alias"))?;

        assert_import_rejected(&package, &destination, &engine, digest_config);
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn import_rejects_symlinked_payload_parent() -> bsmr_error::Result<()> {
        let (temporary, package, destination, digest_config, engine) = exported_fixture()?;
        let payload = package.join(&manifest(&package)?.files[0].path);
        let prefix = payload.parent().expect("payload has a prefix");
        let outside = temporary.path().join("outside-prefix");
        fs::rename(prefix, &outside)?;
        std::os::unix::fs::symlink(&outside, prefix)?;

        assert_import_rejected(&package, &destination, &engine, digest_config);
        Ok(())
    }

    #[test]
    fn import_rejects_duplicate_manifest_paths() -> bsmr_error::Result<()> {
        let (_temporary, package, destination, digest_config, engine) = exported_fixture()?;
        let mut manifest = manifest(&package)?;
        manifest.files.insert(1, manifest.files[0].clone());
        fs::write(
            package.join(super::MANIFEST),
            serde_json::to_vec_pretty(&manifest)?,
        )?;

        assert_import_rejected(&package, &destination, &engine, digest_config);
        Ok(())
    }
}
