//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Shares immutable HTTP blobs across repositories without trusting cached bytes.

use std::fs;
use std::fs::File;
use std::fs::OpenOptions;
use std::io;
use std::io::BufReader;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;

use bsmr_common::cas_digest::CasDigestConfig;
use bsmr_common::file_ops::metadata::FileDigest;
use bsmr_common::file_ops::metadata::TrackedFileDigest;
use digest::DynDigest;
use sha1::Digest;
use sha1::Sha1;
use sha2::Sha256;

use super::http::Checksum;

static TEMPORARY_ID: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, bsmr_error::Error)]
#[bsmr(tag = Environment)]
enum HttpCacheError {
    #[error("HTTP cache {operation} failed for '{}': {source}", path.display())]
    Io {
        operation: &'static str,
        path: PathBuf,
        #[source]
        source: io::Error,
    },
    #[error(
        "HTTP cache checksum mismatch for '{}': expected {expected}, got {obtained}",
        path.display()
    )]
    Checksum {
        path: PathBuf,
        expected: String,
        obtained: String,
    },
    #[error("HTTP cache entry '{}' is not a regular file", _0.display())]
    NotFile(PathBuf),
    #[error("BSMR could not determine the user cache directory")]
    MissingUserCacheDirectory,
    #[error("BSMR_HTTP_CACHE_DIR must be absolute, got '{}'", _0.display())]
    RelativeCacheDirectory(PathBuf),
}

/// Resolves one checksum to its repository-independent cache location.
pub(super) fn path(checksum: &Checksum) -> bsmr_error::Result<PathBuf> {
    let root = match std::env::var_os("BSMR_HTTP_CACHE_DIR") {
        Some(value) => PathBuf::from(value),
        None => dirs::cache_dir()
            .ok_or(HttpCacheError::MissingUserCacheDirectory)?
            .join("bsmr/http-v1"),
    };
    if !root.is_absolute() {
        return Err(HttpCacheError::RelativeCacheDirectory(root).into());
    }
    Ok(path_in(&root, checksum))
}

/// Fans immutable keys out by algorithm and leading digest bytes.
fn path_in(root: &Path, checksum: &Checksum) -> PathBuf {
    let (algorithm, digest) = checksum
        .sha256()
        .map(|digest| ("sha256", digest))
        .or_else(|| checksum.sha1().map(|digest| ("sha1", digest)))
        .expect("Checksum construction rejects empty digests");
    root.join(algorithm).join(&digest[..2]).join(digest)
}

/// Publishes a verified download atomically under its immutable cache key.
pub(super) fn publish(cache: &Path, source: &Path) -> bsmr_error::Result<()> {
    let parent = cache
        .parent()
        .expect("content-addressed cache keys always have a parent");
    fs::create_dir_all(parent).map_err(|source| io_error("create directory", parent, source))?;
    let (temporary, mut output) = create_temporary(cache)?;
    let mut input = File::open(source).map_err(|error| io_error("open", source, error))?;
    io::copy(&mut input, &mut output).map_err(|error| io_error("write", &temporary, error))?;
    output
        .sync_all()
        .map_err(|error| io_error("sync", &temporary, error))?;
    drop(output);
    match fs::rename(&temporary, cache) {
        Ok(()) => Ok(()),
        Err(_) if cache.is_file() => {
            fs::remove_file(&temporary).map_err(|error| io_error("remove", &temporary, error))?;
            Ok(())
        }
        Err(source) => {
            let _ignored = fs::remove_file(&temporary);
            Err(io_error("publish", cache, source))
        }
    }
}

/// Copies and revalidates one cached blob, returning `None` on a clean miss.
pub(super) fn restore(
    cache: &Path,
    destination: &Path,
    checksum: &Checksum,
    digest_config: CasDigestConfig,
) -> bsmr_error::Result<Option<TrackedFileDigest>> {
    let metadata = match fs::symlink_metadata(cache) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(io_error("inspect", cache, error)),
    };
    if !metadata.file_type().is_file() {
        return Err(HttpCacheError::NotFile(cache.to_owned()).into());
    }
    fs::copy(cache, destination).map_err(|error| io_error("restore", cache, error))?;
    match verify(destination, checksum, digest_config) {
        Ok(digest) => Ok(Some(digest)),
        Err(error) => {
            fs::remove_file(destination)
                .map_err(|source| io_error("remove corrupt restoration", destination, source))?;
            fs::remove_file(cache)
                .map_err(|source| io_error("remove corrupt cache entry", cache, source))?;
            Err(error)
        }
    }
}

/// Allocates a collision-free sibling used for atomic cache publication.
fn create_temporary(cache: &Path) -> bsmr_error::Result<(PathBuf, File)> {
    loop {
        let id = TEMPORARY_ID.fetch_add(1, Ordering::Relaxed);
        let temporary = cache.with_extension(format!("tmp.{}.{id}", std::process::id()));
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

/// Recomputes both the action digest and every declared checksum from disk.
fn verify(
    path: &Path,
    checksum: &Checksum,
    digest_config: CasDigestConfig,
) -> bsmr_error::Result<TrackedFileDigest> {
    let mut digester = FileDigest::digester(digest_config);
    let mut sha1 = checksum
        .sha1()
        .map(|_| Box::new(Sha1::new()) as Box<dyn DynDigest>);
    let mut sha256 = checksum
        .sha256()
        .map(|_| Box::new(Sha256::new()) as Box<dyn DynDigest>);
    let mut input =
        BufReader::new(File::open(path).map_err(|error| io_error("open", path, error))?);
    let mut buffer = vec![0_u8; 64 * 1024].into_boxed_slice();
    loop {
        let length = input
            .read(&mut buffer)
            .map_err(|error| io_error("read", path, error))?;
        if length == 0 {
            break;
        }
        let bytes = &buffer[..length];
        digester.update(bytes);
        for validator in [&mut sha1, &mut sha256].into_iter().flatten() {
            validator.update(bytes);
        }
    }
    validate(path, checksum.sha1(), sha1)?;
    validate(path, checksum.sha256(), sha256)?;
    Ok(TrackedFileDigest::new(digester.finalize(), digest_config))
}

/// Compares one optional checksum without creating an unchecked cache mode.
fn validate(
    path: &Path,
    expected: Option<&str>,
    validator: Option<Box<dyn DynDigest>>,
) -> bsmr_error::Result<()> {
    let (expected, validator) = match (expected, validator) {
        (Some(expected), Some(validator)) => (expected, validator),
        (None, None) => return Ok(()),
        _ => unreachable!("checksum validators mirror declared checksums"),
    };
    let obtained = hex::encode(validator.finalize());
    if obtained != expected {
        return Err(HttpCacheError::Checksum {
            path: path.to_owned(),
            expected: expected.to_owned(),
            obtained,
        }
        .into());
    }
    Ok(())
}

/// Wraps filesystem failures with the cache operation and exact path.
fn io_error(operation: &'static str, path: &Path, source: io::Error) -> bsmr_error::Error {
    HttpCacheError::Io {
        operation,
        path: path.to_owned(),
        source,
    }
    .into()
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;
    use std::sync::Arc;

    use bsmr_common::cas_digest::testing;
    use bsmr_fs::paths::abs_path::AbsPath;

    use super::path_in;
    use super::publish;
    use super::restore;
    use crate::materialize::http::Checksum;

    fn checksum() -> Checksum {
        Checksum::Both {
            sha1: Arc::from("8843d7f92416211de9ebb963ff4ce28125932878"),
            sha256: Arc::from("c3ab8ff13720e8ad9047dd39466b3c8974e592c2fa383d4a3960714caef0c4f2"),
        }
    }

    #[test]
    fn verified_blob_round_trips_between_repositories() -> bsmr_error::Result<()> {
        let temporary = tempfile::tempdir()?;
        let root = AbsPath::new(temporary.path())?;
        let source = root.join("source");
        let cache = root.join("cache");
        let destination = root.join("destination");
        fs::write(&source, b"foobar")?;

        publish(&cache, &source)?;
        let digest = restore(&cache, &destination, &checksum(), testing::blake3())?
            .expect("published blob must be restored");

        assert_eq!(fs::read(destination)?, b"foobar");
        assert_eq!(digest.size(), 6);
        Ok(())
    }

    #[test]
    fn corrupt_blob_fails_closed() -> bsmr_error::Result<()> {
        let temporary = tempfile::tempdir()?;
        let root = AbsPath::new(temporary.path())?;
        let cache = root.join("cache");
        let destination = root.join("destination");
        fs::write(&cache, b"not foobar")?;

        let error = restore(&cache, &destination, &checksum(), testing::blake3())
            .expect_err("cache corruption must fail the build");

        assert!(error.to_string().contains("checksum"));
        assert!(!destination.exists());
        assert!(!cache.exists());
        Ok(())
    }

    #[test]
    fn absent_blob_is_a_cache_miss() -> bsmr_error::Result<()> {
        let temporary = tempfile::tempdir()?;
        let root = AbsPath::new(temporary.path())?;

        assert!(
            restore(
                &root.join("missing"),
                &root.join("destination"),
                &checksum(),
                testing::blake3(),
            )?
            .is_none()
        );
        Ok(())
    }

    #[test]
    fn strongest_checksum_defines_the_cache_identity() {
        assert_eq!(
            path_in(Path::new("/cache"), &checksum()),
            Path::new(
                "/cache/sha256/c3/c3ab8ff13720e8ad9047dd39466b3c8974e592c2fa383d4a3960714caef0c4f2"
            )
        );
    }
}
