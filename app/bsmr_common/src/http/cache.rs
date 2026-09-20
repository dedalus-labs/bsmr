//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Shares immutable HTTP blobs across repositories without trusting cached bytes.

use std::fs;
use std::fs::File;
use std::io;
use std::io::BufReader;
use std::io::Read;
use std::io::Seek;
use std::path::Path;
use std::path::PathBuf;

use digest::DynDigest;
use sha1::Digest;
use sha1::Sha1;
use sha2::Sha256;

use super::checksum::Checksum;
use crate::cas_digest::CasDigestConfig;
use crate::file_ops::metadata::FileDigest;
use crate::file_ops::metadata::TrackedFileDigest;

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
pub fn path(checksum: &Checksum) -> bsmr_error::Result<PathBuf> {
    let root = match std::env::var_os("BSMR_HTTP_CACHE_DIR") {
        Some(value) => PathBuf::from(value),
        None => dirs::cache_dir()
            .ok_or(HttpCacheError::MissingUserCacheDirectory)?
            .join("bsmr/http-v1"),
    };
    if !root.is_absolute() {
        return Err(HttpCacheError::RelativeCacheDirectory(root).into());
    }
    let checksum = Checksum::new(checksum.sha1(), checksum.sha256())?;
    Ok(path_in(&root, &checksum))
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

/// Copies an acquired blob into an owned temporary file, verifies it, then publishes atomically.
/// Returns the digest and byte size of the published copy. Rejection preserves the existing entry.
pub fn import(
    cache: &Path,
    source: &Path,
    checksum: &Checksum,
    digest_config: CasDigestConfig,
) -> bsmr_error::Result<TrackedFileDigest> {
    let parent = cache
        .parent()
        .expect("content-addressed cache keys always have a parent");
    fs::create_dir_all(parent).map_err(|source| io_error("create directory", parent, source))?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)
        .map_err(|source| io_error("create temporary", parent, source))?;
    let mut input = File::open(source).map_err(|error| io_error("open", source, error))?;
    io::copy(&mut input, &mut temporary)
        .map_err(|error| io_error("write", temporary.path(), error))?;
    let digest = verify(
        temporary.path(),
        temporary.as_file(),
        checksum,
        digest_config,
    )?;
    temporary
        .as_file()
        .sync_all()
        .map_err(|error| io_error("sync", temporary.path(), error))?;
    temporary
        .persist(cache)
        .map_err(|error| io_error("publish", cache, error.error))?;
    Ok(digest)
}

/// Copies and revalidates one cached blob, returning `None` on a clean miss.
pub fn restore(
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
    let copied = File::open(destination).map_err(|error| io_error("open", destination, error))?;
    match verify(destination, &copied, checksum, digest_config) {
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

/// Recomputes both the action digest and every declared checksum from disk.
fn verify(
    path: &Path,
    mut input: &File,
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
    input
        .rewind()
        .map_err(|error| io_error("seek", path, error))?;
    let mut input = BufReader::new(input);
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

    use bsmr_fs::paths::abs_path::AbsPath;

    use super::import;
    use super::path;
    use super::path_in;
    use super::restore;
    use crate::cas_digest::testing;
    use crate::http::checksum::Checksum;

    /// Declares both digests for the test blob.
    fn checksum() -> Checksum {
        Checksum::Both {
            sha1: Arc::from("8843d7f92416211de9ebb963ff4ce28125932878"),
            sha256: Arc::from("c3ab8ff13720e8ad9047dd39466b3c8974e592c2fa383d4a3960714caef0c4f2"),
        }
    }

    /// The imported copy survives source changes and restores with its exact digest.
    #[test]
    fn verified_blob_round_trips_between_repositories() -> bsmr_error::Result<()> {
        let temporary = tempfile::tempdir()?;
        let root = AbsPath::new(temporary.path())?;
        let source = root.join("source");
        let cache = root.join("cache");
        let destination = root.join("destination");
        fs::write(&source, b"foobar")?;

        let imported = import(&cache, &source, &checksum(), testing::blake3())?;
        fs::write(&source, b"changed after import")?;
        let digest = restore(&cache, &destination, &checksum(), testing::blake3())?
            .expect("published blob must be restored");

        assert_eq!(fs::read(destination)?, b"foobar");
        assert_eq!(digest.size(), 6);
        assert_eq!(digest, imported);
        Ok(())
    }

    /// A rejected import preserves the last verified entry and removes its temporary file.
    #[test]
    fn invariant_import_rejects_corruption_before_publication() -> bsmr_error::Result<()> {
        let temporary = tempfile::tempdir()?;
        let source = temporary.path().join("source");
        let cache = temporary.path().join("cache/blob");
        fs::write(&source, b"foobar")?;
        import(&cache, &source, &checksum(), testing::blake3())?;
        fs::write(&source, b"corrupt acquired blob")?;

        let error = import(&cache, &source, &checksum(), testing::blake3())
            .expect_err("corruption must be rejected before publication");

        assert!(error.to_string().contains("checksum"));
        assert_eq!(fs::read(&cache)?, b"foobar");
        assert_eq!(fs::read_dir(cache.parent().unwrap())?.count(), 1);
        Ok(())
    }

    /// Invalid public enum values cannot become cache paths.
    #[test]
    fn invariant_cache_keys_require_validated_digests() {
        assert!(path(&Checksum::Sha256(Arc::from("short"))).is_err());
    }

    /// Cached corruption fails loudly and removes the rejected bytes.
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

    /// An absent immutable blob is the only clean cache miss.
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

    /// A blob with both checksums shares the SHA-256 cache entry.
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
