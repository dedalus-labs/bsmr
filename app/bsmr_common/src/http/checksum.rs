//===----------------------------------------------------------------------===//
// Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc
// Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

// Validates the checksums shared by acquisition and materialization.

use std::sync::Arc;

use allocative::Allocative;
use dupe::Dupe;
use pagable::Pagable;

use crate::cas_digest::SHA1_SIZE;
use crate::cas_digest::SHA256_SIZE;

/// Declares the digest algorithms that an acquired blob must satisfy.
#[derive(Debug, Clone, Dupe, Allocative, Pagable)]
pub enum Checksum {
    Sha1(Arc<str>),
    Sha256(Arc<str>),
    Both { sha1: Arc<str>, sha256: Arc<str> },
}

#[derive(bsmr_error::Error, Debug)]
#[bsmr(tag = Input)]
enum DownloadFileError {
    #[error("Must pass in at least one checksum (e.g. `sha1 = ...`)")]
    MissingChecksum,
    #[error("Invalid digest for `{digest_type}` argument, expected length of {expected_len} but got {}, digest `{digest}`", digest.len())]
    InvalidDigestLength {
        digest: String,
        expected_len: usize,
        digest_type: &'static str,
    },
    #[error(
        "Invalid digest for `{digest_type}` argument, expected 0-9 a-z hex characters, but got `{bad_char}`, digest `{digest}`"
    )]
    InvalidDigestCharacter {
        digest: String,
        bad_char: char,
        digest_type: &'static str,
    },
}

impl Checksum {
    /// Validates hexadecimal digest lengths and normalizes their case.
    pub fn new(sha1: Option<&str>, sha256: Option<&str>) -> bsmr_error::Result<Self> {
        /// Accepts ASCII hexadecimal digits in either case.
        fn is_hex_digit(x: char) -> bool {
            let x = x.to_ascii_lowercase();
            x.is_ascii_digit() || ('a'..='f').contains(&x)
        }

        /// Validates one optional digest before storing it.
        fn validate_digest(
            digest: Option<&str>,
            digest_len: usize,
            digest_type: &'static str,
        ) -> bsmr_error::Result<Option<Arc<str>>> {
            match digest {
                None => Ok(None),
                Some(digest) => {
                    let expected_len = digest_len * 2;
                    if digest.len() != expected_len {
                        return Err(DownloadFileError::InvalidDigestLength {
                            digest: digest.to_owned(),
                            expected_len,
                            digest_type,
                        }
                        .into());
                    }
                    if let Some(bad_char) = digest.chars().find(|x| !is_hex_digit(*x)) {
                        return Err(DownloadFileError::InvalidDigestCharacter {
                            digest: digest.to_owned(),
                            bad_char,
                            digest_type,
                        }
                        .into());
                    }
                    Ok(Some(Arc::from(digest.to_ascii_lowercase())))
                }
            }
        }

        match (
            validate_digest(sha1, SHA1_SIZE, "sha1")?,
            validate_digest(sha256, SHA256_SIZE, "sha256")?,
        ) {
            (Some(sha1), None) => Ok(Checksum::Sha1(sha1)),
            (None, Some(sha256)) => Ok(Checksum::Sha256(sha256)),
            (Some(sha1), Some(sha256)) => Ok(Checksum::Both { sha1, sha256 }),
            (None, None) => Err(DownloadFileError::MissingChecksum.into()),
        }
    }

    /// Returns the declared SHA-1 digest.
    pub fn sha1(&self) -> Option<&str> {
        match self {
            Self::Sha1(sha1) => Some(sha1),
            Self::Sha256(..) => None,
            Self::Both { sha1, .. } => Some(sha1),
        }
    }

    /// Returns the declared SHA-256 digest.
    pub fn sha256(&self) -> Option<&str> {
        match self {
            Self::Sha1(..) => None,
            Self::Sha256(sha256) => Some(sha256),
            Self::Both { sha256, .. } => Some(sha256),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Checksum;

    /// Constructors normalize valid hex and reject missing or malformed digests.
    #[test]
    fn invariant_checksum_construction_validates_identity() {
        let checksum = Checksum::new(Some(&"AB".repeat(20)), Some(&"CD".repeat(32))).unwrap();
        assert_eq!(checksum.sha1(), Some("ab".repeat(20).as_str()));
        assert_eq!(checksum.sha256(), Some("cd".repeat(32).as_str()));
        assert!(Checksum::new(None, None).is_err());
        for digest in [
            String::new(),
            "a".repeat(63),
            "a".repeat(65),
            "g".repeat(64),
        ] {
            assert!(Checksum::new(None, Some(&digest)).is_err());
        }
    }
}
