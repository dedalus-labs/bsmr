//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Stores action results and immutable blobs in a process-safe local AC/CAS.

use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::io;
use std::io::Read;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use bsmr_common::file_ops::metadata::FileDigest;
use bsmr_common::file_ops::metadata::TrackedFileDigest;
use prost::Message;
use serde::Deserialize;
use serde::Serialize;
use sha2::Digest;
use sha2::Sha256;

use crate::digest::CasDigestFromReExt;
use crate::digest::CasDigestToReExt;
use crate::digest_config::DigestConfig;
use crate::execute::action_digest::ActionDigest;

static TEMPORARY_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, bsmr_error::Error)]
#[bsmr(tag = Environment)]
enum LocalCacheError {
    #[error("Local cache {operation} failed for '{}': {source}", path.display())]
    Io {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error("Local cache could not decode action result '{}': {source}", path.display())]
    DecodeAction {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("Local cache could not decode output tree '{}': {source}", path.display())]
    DecodeTree {
        path: PathBuf,
        #[source]
        source: prost::DecodeError,
    },
    #[error(
        "Local cache digest mismatch for '{}': expected {}, got {}",
        path.display(),
        expected,
        obtained
    )]
    Digest {
        path: PathBuf,
        expected: String,
        obtained: String,
    },
    #[error(
        "Local cache size mismatch for '{}': expected {} bytes, got {}",
        path.display(),
        expected,
        obtained
    )]
    Size {
        path: PathBuf,
        expected: i64,
        obtained: u64,
    },
    #[error("Local cache entry '{}' is not a regular file", _0.display())]
    NotFile(PathBuf),
    #[error("BSMR could not determine the user cache directory")]
    MissingUserCacheDirectory,
    #[error("BSMR_LOCAL_CACHE_DIR must be absolute, got '{}'", _0.display())]
    RelativeCacheDirectory(PathBuf),
    #[error("Local cache digest has a negative size: {0}")]
    NegativeSize(i64),
    #[error("Local cache digest declares {declared}, but its hash parses as {parsed}")]
    DigestAlgorithm { declared: String, parsed: String },
    #[error("Local cache output tree has no root directory")]
    MissingTreeRoot,
    #[error("Local cache output tree contains a file without a digest")]
    MissingFileDigest,
}

/// A content digest persisted in the local action-result format.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LocalDigest {
    pub algorithm: String,
    pub hash: String,
    pub size: i64,
}

impl LocalDigest {
    /// Converts a tracked file digest to the stable local representation.
    pub fn from_file(digest: &TrackedFileDigest) -> Self {
        let digest_re = digest.to_re();
        Self {
            algorithm: digest.data().raw_digest().algorithm().to_string(),
            hash: digest_re.hash,
            size: digest_re.size_in_bytes,
        }
    }

    /// Converts the stable local representation to an RE digest.
    pub fn to_re(&self) -> remote_execution::TDigest {
        remote_execution::TDigest {
            hash: self.hash.clone(),
            size_in_bytes: self.size,
            ..Default::default()
        }
    }

    /// Parses and validates the digest against the active digest configuration.
    pub fn to_file_digest(
        &self,
        digest_config: DigestConfig,
    ) -> bsmr_error::Result<TrackedFileDigest> {
        let digest = FileDigest::from_re(&self.to_re(), digest_config)?;
        let parsed = digest.raw_digest().algorithm().to_string();
        if parsed != self.algorithm {
            return Err(LocalCacheError::DigestAlgorithm {
                declared: self.algorithm.clone(),
                parsed,
            }
            .into());
        }
        Ok(TrackedFileDigest::new(
            digest,
            digest_config.cas_digest_config(),
        ))
    }
}

/// A file output contained directly in an action result.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LocalOutputFile {
    pub path: String,
    pub digest: LocalDigest,
    pub executable: bool,
}

/// A directory output represented by an RE Tree blob in the CAS.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct LocalOutputDirectory {
    pub path: String,
    pub tree_digest: LocalDigest,
}

/// The local action-cache manifest. All byte payloads live in the CAS.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct LocalActionResult {
    pub output_files: Vec<LocalOutputFile>,
    pub output_directories: Vec<LocalOutputDirectory>,
    pub stdout: Option<LocalDigest>,
    pub stderr: Option<LocalDigest>,
}

/// A repository-independent local action cache.
pub struct LocalActionCache {
    root: PathBuf,
}

impl LocalActionCache {
    /// Opens the default user-level cache.
    pub fn open() -> bsmr_error::Result<Self> {
        let root = match std::env::var_os("BSMR_LOCAL_CACHE_DIR") {
            Some(root) => PathBuf::from(root),
            None => dirs::cache_dir()
                .ok_or(LocalCacheError::MissingUserCacheDirectory)?
                .join("bsmr/action-v1"),
        };
        Self::at(root)
    }

    /// Opens a cache rooted at `root`.
    pub fn at(root: PathBuf) -> bsmr_error::Result<Self> {
        if !root.is_absolute() {
            return Err(LocalCacheError::RelativeCacheDirectory(root).into());
        }
        Ok(Self { root })
    }

    /// Returns an action result only when every referenced CAS object exists.
    pub fn action_result(
        &self,
        action: &ActionDigest,
    ) -> bsmr_error::Result<Option<LocalActionResult>> {
        let path = self.action_path(action);
        let bytes = match read_regular_file(&path)? {
            Some(bytes) => bytes,
            None => return Ok(None),
        };
        let result: LocalActionResult =
            serde_json::from_slice(&bytes).map_err(|source| LocalCacheError::DecodeAction {
                path: path.clone(),
                source,
            })?;
        if self.has_complete_closure(&result)? {
            Ok(Some(result))
        } else {
            Ok(None)
        }
    }

    /// Atomically publishes an action result after its CAS closure.
    pub fn publish_action_result(
        &self,
        action: &ActionDigest,
        result: &LocalActionResult,
    ) -> bsmr_error::Result<()> {
        let path = self.action_path(action);
        let bytes = serde_json::to_vec(result).map_err(|source| LocalCacheError::DecodeAction {
            path: path.clone(),
            source,
        })?;
        atomic_publish(&path, |output| {
            output
                .write_all(&bytes)
                .map_err(|source| io_error("write", &path, source))
        })
    }

    /// Atomically publishes verified bytes under their content digest.
    pub fn publish_bytes(
        &self,
        digest: &TrackedFileDigest,
        bytes: &[u8],
        digest_config: DigestConfig,
    ) -> bsmr_error::Result<()> {
        let obtained = FileDigest::from_content(bytes, digest_config.cas_digest_config());
        if &obtained != digest.data() {
            return Err(LocalCacheError::Digest {
                path: self.blob_path(&LocalDigest::from_file(digest)),
                expected: digest.to_string(),
                obtained: obtained.to_string(),
            }
            .into());
        }
        let key = LocalDigest::from_file(digest);
        let path = self.blob_path(&key);
        atomic_publish_immutable(&path, digest.to_re().size_in_bytes, |output| {
            output
                .write_all(bytes)
                .map_err(|source| io_error("write", &path, source))
        })
    }

    /// Atomically publishes a verified file under its content digest.
    pub fn publish_file(
        &self,
        digest: &TrackedFileDigest,
        source: &Path,
        digest_config: DigestConfig,
    ) -> bsmr_error::Result<()> {
        let key = LocalDigest::from_file(digest);
        let path = self.blob_path(&key);
        if validate_blob_path(&path, digest.to_re().size_in_bytes)? {
            return Ok(());
        }
        let mut input = File::open(source).map_err(|error| io_error("open", source, error))?;
        let mut digester = FileDigest::digester(digest_config.cas_digest_config());
        atomic_publish_immutable(&path, digest.to_re().size_in_bytes, |output| {
            let mut buffer = [0_u8; 64 * 1024];
            loop {
                let length = input
                    .read(&mut buffer)
                    .map_err(|error| io_error("read", source, error))?;
                if length == 0 {
                    break;
                }
                digester.update(&buffer[..length]);
                output
                    .write_all(&buffer[..length])
                    .map_err(|error| io_error("write", &path, error))?;
            }
            let obtained = digester.finalize();
            if &obtained != digest.data() {
                return Err(LocalCacheError::Digest {
                    path: source.to_owned(),
                    expected: digest.to_string(),
                    obtained: obtained.to_string(),
                }
                .into());
            }
            Ok(())
        })
    }

    /// Reads and verifies one CAS object, returning `None` only for a clean miss.
    pub fn read_blob(
        &self,
        digest: &LocalDigest,
        digest_config: DigestConfig,
    ) -> bsmr_error::Result<Option<Vec<u8>>> {
        let bytes = self.read_blob_unverified(digest)?;
        if let Some(bytes) = &bytes {
            let expected = digest.to_file_digest(digest_config)?;
            validate_content(&self.blob_path(digest), &expected, bytes, digest_config)?;
        }
        Ok(bytes)
    }

    fn read_blob_unverified(&self, digest: &LocalDigest) -> bsmr_error::Result<Option<Vec<u8>>> {
        let path = self.blob_path(digest);
        let bytes = match read_regular_file(&path)? {
            Some(bytes) => bytes,
            None => return Ok(None),
        };
        validate_size(&path, digest.size, bytes.len() as u64)?;
        Ok(Some(bytes))
    }

    /// Copies one CAS object to `destination` and validates its size.
    pub fn restore_blob(
        &self,
        digest: &TrackedFileDigest,
        destination: &Path,
        digest_config: DigestConfig,
    ) -> bsmr_error::Result<()> {
        let key = LocalDigest::from_file(digest);
        let source = self.blob_path(&key);
        if !validate_blob_path(&source, key.size)? {
            return Err(io_error(
                "restore missing blob",
                &source,
                io::Error::from(io::ErrorKind::NotFound),
            ));
        }
        let parent = destination
            .parent()
            .expect("restored output paths always have a parent");
        fs::create_dir_all(parent).map_err(|error| io_error("create directory", parent, error))?;
        let (temporary, output) = create_temporary(destination)?;
        drop(output);
        fs::copy(&source, &temporary).map_err(|error| io_error("restore", destination, error))?;
        let obtained = fs::metadata(&temporary)
            .map_err(|error| io_error("inspect", &temporary, error))?
            .len();
        if let Err(error) = validate_size(&temporary, key.size, obtained) {
            let _ignored = fs::remove_file(&temporary);
            return Err(error);
        }
        let input = File::open(&temporary).map_err(|error| io_error("open", &temporary, error))?;
        let obtained = FileDigest::from_reader(input, digest_config.cas_digest_config())?;
        if let Err(error) = validate_digest(&temporary, digest, &obtained) {
            let _ignored = fs::remove_file(&temporary);
            return Err(error);
        }
        fs::rename(&temporary, destination)
            .map_err(|error| io_error("publish restoration", destination, error))?;
        Ok(())
    }

    fn has_complete_closure(&self, result: &LocalActionResult) -> bsmr_error::Result<bool> {
        for digest in result
            .output_files
            .iter()
            .map(|file| &file.digest)
            .chain(result.stdout.iter())
            .chain(result.stderr.iter())
        {
            if !validate_blob_path(&self.blob_path(digest), digest.size)? {
                return Ok(false);
            }
        }
        for directory in &result.output_directories {
            let digest = &directory.tree_digest;
            let Some(tree_bytes) = self.read_blob_unverified(digest)? else {
                return Ok(false);
            };
            let tree = remote_execution::Tree::decode(tree_bytes.as_slice()).map_err(|source| {
                LocalCacheError::DecodeTree {
                    path: self.blob_path(digest),
                    source,
                }
            })?;
            if tree.root.is_none() {
                return Err(LocalCacheError::MissingTreeRoot.into());
            }
            for file in tree
                .root
                .iter()
                .chain(tree.children.iter())
                .flat_map(|directory| &directory.files)
            {
                let digest = file
                    .digest
                    .as_ref()
                    .ok_or(LocalCacheError::MissingFileDigest)?;
                let digest = LocalDigest {
                    algorithm: directory.tree_digest.algorithm.clone(),
                    hash: digest.hash.clone(),
                    size: digest.size_bytes,
                };
                if !validate_blob_path(&self.blob_path(&digest), digest.size)? {
                    tracing::debug!(digest = ?digest, "local cache tree references a missing blob");
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    fn action_path(&self, action: &ActionDigest) -> PathBuf {
        keyed_path(
            &self.root.join("ac"),
            format!("{}:{}", action.raw_digest().algorithm(), action).as_bytes(),
        )
    }

    fn blob_path(&self, digest: &LocalDigest) -> PathBuf {
        keyed_path(
            &self.root.join("cas"),
            format!("{}:{}:{}", digest.algorithm, digest.hash, digest.size).as_bytes(),
        )
    }
}

fn keyed_path(root: &Path, key: &[u8]) -> PathBuf {
    let digest = hex::encode(Sha256::digest(key));
    root.join(&digest[..2]).join(digest)
}

fn read_regular_file(path: &Path) -> bsmr_error::Result<Option<Vec<u8>>> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(io_error("inspect", path, error)),
    };
    if !metadata.file_type().is_file() {
        return Err(LocalCacheError::NotFile(path.to_owned()).into());
    }
    fs::read(path)
        .map(Some)
        .map_err(|error| io_error("read", path, error))
}

fn validate_blob_path(path: &Path, expected: i64) -> bsmr_error::Result<bool> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(io_error("inspect", path, error)),
    };
    if !metadata.file_type().is_file() {
        return Err(LocalCacheError::NotFile(path.to_owned()).into());
    }
    validate_size(path, expected, metadata.len())?;
    Ok(true)
}

fn validate_size(path: &Path, expected: i64, obtained: u64) -> bsmr_error::Result<()> {
    let expected_u64: u64 = expected
        .try_into()
        .map_err(|_| LocalCacheError::NegativeSize(expected))?;
    if expected_u64 != obtained {
        return Err(LocalCacheError::Size {
            path: path.to_owned(),
            expected,
            obtained,
        }
        .into());
    }
    Ok(())
}

fn validate_content(
    path: &Path,
    expected: &TrackedFileDigest,
    bytes: &[u8],
    digest_config: DigestConfig,
) -> bsmr_error::Result<()> {
    let obtained = FileDigest::from_content(bytes, digest_config.cas_digest_config());
    validate_digest(path, expected, &obtained)
}

fn validate_digest(
    path: &Path,
    expected: &TrackedFileDigest,
    obtained: &FileDigest,
) -> bsmr_error::Result<()> {
    if obtained != expected.data() {
        return Err(LocalCacheError::Digest {
            path: path.to_owned(),
            expected: expected.to_string(),
            obtained: obtained.to_string(),
        }
        .into());
    }
    Ok(())
}

fn atomic_publish_immutable(
    path: &Path,
    expected_size: i64,
    write: impl FnOnce(&mut File) -> bsmr_error::Result<()>,
) -> bsmr_error::Result<()> {
    if validate_blob_path(path, expected_size)? {
        return Ok(());
    }
    atomic_publish_inner(path, write, false)
}

fn atomic_publish(
    path: &Path,
    write: impl FnOnce(&mut File) -> bsmr_error::Result<()>,
) -> bsmr_error::Result<()> {
    atomic_publish_inner(path, write, true)
}

fn atomic_publish_inner(
    path: &Path,
    write: impl FnOnce(&mut File) -> bsmr_error::Result<()>,
    durable: bool,
) -> bsmr_error::Result<()> {
    let parent = path.parent().expect("cache entries always have a parent");
    fs::create_dir_all(parent).map_err(|error| io_error("create directory", parent, error))?;
    let (temporary, mut output) = create_temporary(path)?;
    if let Err(error) = write(&mut output) {
        let _ignored = fs::remove_file(&temporary);
        return Err(error);
    }
    if durable {
        output
            .sync_all()
            .map_err(|error| io_error("sync", &temporary, error))?;
    }
    drop(output);
    fs::rename(&temporary, path).map_err(|error| io_error("publish", path, error))?;
    if durable {
        File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| io_error("sync directory", parent, error))?;
    }
    Ok(())
}

fn create_temporary(path: &Path) -> bsmr_error::Result<(PathBuf, File)> {
    loop {
        let id = TEMPORARY_ID.fetch_add(1, Ordering::Relaxed);
        let temporary = path.with_extension(format!("tmp.{}.{id}", std::process::id()));
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
        {
            Ok(file) => return Ok((temporary, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(io_error("create temporary", &temporary, error)),
        }
    }
}

fn io_error(
    operation: &'static str,
    path: impl AsRef<Path>,
    source: io::Error,
) -> bsmr_error::Error {
    LocalCacheError::Io {
        operation,
        path: path.as_ref().to_owned(),
        source,
    }
    .into()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use bsmr_common::file_ops::metadata::TrackedFileDigest;
    use prost::Message;

    use super::LocalActionCache;
    use super::LocalActionResult;
    use super::LocalDigest;
    use super::LocalOutputDirectory;
    use super::LocalOutputFile;
    use crate::digest::CasDigestToReExt;
    use crate::digest_config::DigestConfig;
    use crate::execute::action_digest::ActionDigest;

    fn fixture() -> (
        tempfile::TempDir,
        LocalActionCache,
        DigestConfig,
        ActionDigest,
        TrackedFileDigest,
        LocalActionResult,
    ) {
        let temporary = tempfile::tempdir().expect("temporary directory");
        let cache = LocalActionCache::at(temporary.path().to_owned()).expect("cache");
        let digest_config = DigestConfig::testing_default();
        let action = ActionDigest::from_content(b"action", digest_config.cas_digest_config());
        let output =
            TrackedFileDigest::from_content(b"cached output", digest_config.cas_digest_config());
        let result = LocalActionResult {
            output_files: vec![LocalOutputFile {
                path: "buck-out/output".to_owned(),
                digest: LocalDigest::from_file(&output),
                executable: false,
            }],
            ..Default::default()
        };
        (temporary, cache, digest_config, action, output, result)
    }

    #[test]
    fn complete_action_round_trips() -> bsmr_error::Result<()> {
        let (_temporary, cache, digest_config, action, output, result) = fixture();

        cache.publish_bytes(&output, b"cached output", digest_config)?;
        cache.publish_action_result(&action, &result)?;

        assert_eq!(cache.action_result(&action)?, Some(result));
        Ok(())
    }

    #[test]
    fn dangling_action_result_is_a_miss() -> bsmr_error::Result<()> {
        let (_temporary, cache, _digest_config, action, _output, result) = fixture();

        cache.publish_action_result(&action, &result)?;

        assert!(cache.action_result(&action)?.is_none());
        Ok(())
    }

    #[test]
    fn corrupt_action_result_fails_closed() -> bsmr_error::Result<()> {
        let (temporary, cache, _digest_config, action, _output, _result) = fixture();
        let action_path = cache.action_path(&action);
        fs::create_dir_all(action_path.parent().expect("action parent"))?;
        fs::write(&action_path, b"not json")?;

        let error = match cache.action_result(&action) {
            Ok(_) => panic!("corrupt action result must fail"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("decode"));
        assert!(temporary.path().exists());
        Ok(())
    }

    #[test]
    fn corrupt_blob_fails_closed() -> bsmr_error::Result<()> {
        let (_temporary, cache, digest_config, _action, output, _result) = fixture();
        cache.publish_bytes(&output, b"cached output", digest_config)?;
        fs::write(cache.blob_path(&LocalDigest::from_file(&output)), b"short")?;

        let error = cache
            .restore_blob(&output, &cache.root.join("restored"), digest_config)
            .expect_err("corrupt blob must fail");

        assert!(error.to_string().contains("size"));
        Ok(())
    }

    #[test]
    fn same_size_corrupt_blob_fails_closed() -> bsmr_error::Result<()> {
        let (_temporary, cache, digest_config, _action, output, _result) = fixture();
        cache.publish_bytes(&output, b"cached output", digest_config)?;
        fs::write(
            cache.blob_path(&LocalDigest::from_file(&output)),
            b"poison output",
        )?;

        let error = cache
            .restore_blob(&output, &cache.root.join("restored"), digest_config)
            .expect_err("same-size corruption must fail");

        assert!(error.to_string().contains("digest mismatch"));
        Ok(())
    }

    #[test]
    fn directory_result_requires_its_complete_tree_closure() -> bsmr_error::Result<()> {
        let (_temporary, cache, digest_config, action, _output, _result) = fixture();
        let file =
            TrackedFileDigest::from_content(b"nested output", digest_config.cas_digest_config());
        let tree = remote_execution::Tree {
            root: Some(remote_execution::Directory {
                files: vec![remote_execution::FileNode {
                    name: "nested.txt".to_owned(),
                    digest: Some(file.to_grpc()),
                    ..Default::default()
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let tree_bytes = tree.encode_to_vec();
        let tree_digest =
            TrackedFileDigest::from_content(&tree_bytes, digest_config.cas_digest_config());
        let result = LocalActionResult {
            output_directories: vec![LocalOutputDirectory {
                path: "buck-out/directory".to_owned(),
                tree_digest: LocalDigest::from_file(&tree_digest),
            }],
            ..Default::default()
        };
        cache.publish_bytes(&file, b"nested output", digest_config)?;
        cache.publish_bytes(&tree_digest, &tree_bytes, digest_config)?;
        cache.publish_action_result(&action, &result)?;

        assert_eq!(cache.action_result(&action)?, Some(result));

        fs::remove_file(cache.blob_path(&LocalDigest::from_file(&file)))?;
        assert!(cache.action_result(&action)?.is_none());
        Ok(())
    }

    #[test]
    fn declared_digest_algorithm_must_match_the_hash() -> bsmr_error::Result<()> {
        let (_temporary, _cache, digest_config, _action, output, _result) = fixture();
        let mut digest = LocalDigest::from_file(&output);
        digest.algorithm = "SHA256".to_owned();

        let error = digest
            .to_file_digest(digest_config)
            .expect_err("mismatched digest family must fail");

        assert!(error.to_string().contains("declares SHA256"));
        Ok(())
    }
}
