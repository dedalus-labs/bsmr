//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Verify the bytes transferred from an analyzed input file into isolated storage.

use std::fs;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use std::path::PathBuf;

use bsmr_common::cas_digest::DigestAlgorithmFamily;
use bsmr_common::cas_digest::Digester;
use bsmr_common::file_ops::metadata::FileDigest;
use bsmr_common::file_ops::metadata::FileDigestKind;
use bsmr_common::file_ops::metadata::FileMetadata;
use bsmr_execute::digest_config::DigestConfig;

#[derive(Debug, bsmr_error::Error)]
#[bsmr(tag = Input)]
enum InputError {
    #[error("action input is no longer a regular file: {0:?}")]
    FileType(PathBuf),
    #[error("failed to transfer action input {path:?}: {error}")]
    Transfer {
        /// Source whose transfer failed.
        path: PathBuf,
        /// Preserve the underlying filesystem or transport failure.
        #[source]
        error: std::io::Error,
    },
    #[error("action input {path:?} changed after analysis: expected {expected}, got {actual}")]
    Mutation {
        /// Source that no longer matches the analyzed input.
        path: PathBuf,
        /// Content identity declared by the action.
        expected: FileDigest,
        /// Content identity observed during transfer.
        actual: FileDigest,
    },
}

/// Hash the bytes consumed by either input transport without buffering the file.
pub(super) struct DigestingReader<R> {
    /// Owns the source until the transfer completes.
    inner: R,
    /// Tracks the digest and length of the bytes actually read.
    digester: Digester<FileDigestKind>,
}

impl<R> DigestingReader<R> {
    /// Bind one reader to the analyzed content's digest algorithm.
    pub(super) fn new(inner: R, algorithm: bsmr_common::cas_digest::DigestAlgorithm) -> Self {
        Self {
            inner,
            digester: FileDigest::digester_for_algorithm(algorithm),
        }
    }

    /// Return the digest of the exact bytes consumed by the transport.
    pub(super) fn finish(self) -> FileDigest {
        self.digester.finalize()
    }
}

impl<R: Read> Read for DigestingReader<R> {
    fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
        let read = self.inner.read(buffer)?;
        self.digester.update(&buffer[..read]);
        Ok(read)
    }
}

/// Transfer one file and reject changed, truncated or extended contents before success.
pub(super) fn with_file<T>(
    project: &Path,
    path: &Path,
    metadata: &FileMetadata,
    digest_config: DigestConfig,
    transfer: impl FnOnce(&mut DigestingReader<File>) -> std::io::Result<T>,
) -> bsmr_error::Result<T> {
    let source = project.join(path);
    let io_error = |error| InputError::Transfer {
        path: source.clone(),
        error,
    };
    if !fs::symlink_metadata(&source)
        .map_err(io_error)?
        .file_type()
        .is_file()
    {
        return Err(InputError::FileType(source).into());
    }
    let file = File::open(&source).map_err(io_error)?;
    let algorithm = match metadata.digest.raw_digest().algorithm() {
        DigestAlgorithmFamily::Sha1 => digest_config.cas_digest_config().digest160(),
        DigestAlgorithmFamily::Sha256
        | DigestAlgorithmFamily::Blake3
        | DigestAlgorithmFamily::Blake3Keyed => digest_config.cas_digest_config().digest256(),
    }
    .expect("an input digest algorithm must be enabled in its action configuration");
    let mut verified = DigestingReader::new(file, algorithm);
    let result = transfer(&mut verified).map_err(io_error)?;
    verified.read(&mut [0u8; 1]).map_err(io_error)?;
    let actual = verified.finish();
    if &actual != metadata.digest.data() {
        return Err(InputError::Mutation {
            path: source,
            expected: *metadata.digest.data(),
            actual,
        }
        .into());
    }
    Ok(result)
}
