//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Gives an immutable dependency version set a canonical CAS identity.

//! Gives an immutable dependency version set a canonical CAS identity.
//!
//! Ecosystem adapters encode semantic graph nodes and edges as a Merkle DAG,
//! then provide its canonical root record. This module prefixes that record
//! with a versioned domain and hashes the exact object admitted to Bessemer's
//! existing CAS.

use crate::cas_digest::CasDigest;
use crate::cas_digest::CasDigestConfig;
use crate::cas_digest::CasDigestKind;
use crate::cas_digest::TrackedCasDigest;

const FORMAT: &[u8] = b"bsmr.version-set.v1\0";

/// The canonical CAS object for one complete dependency universe.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VersionSet {
    bytes: Box<[u8]>,
}

impl VersionSet {
    /// Prefixes an ecosystem adapter's canonical graph root with Bessemer's format domain.
    ///
    /// The caller must encode every resolution-affecting node and edge. In
    /// particular, package name and semantic version alone are not sufficient.
    #[must_use]
    pub fn from_canonical_root(root: &[u8]) -> Self {
        let mut bytes = Vec::with_capacity(FORMAT.len() + root.len());
        bytes.extend_from_slice(FORMAT);
        bytes.extend_from_slice(root);
        Self {
            bytes: bytes.into_boxed_slice(),
        }
    }

    /// Returns the exact bytes stored in the CAS.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Computes the typed CAS identity under the repository's digest configuration.
    #[must_use]
    pub fn digest(&self, config: CasDigestConfig) -> VersionSetDigest {
        VersionSetDigest::from_content(self.as_bytes(), config)
    }
}

/// Prevents a version-set root from being confused with an arbitrary CAS blob.
pub struct VersionSetDigestKind {
    _private: (),
}

impl CasDigestKind for VersionSetDigestKind {
    fn empty_digest(_config: CasDigestConfig) -> Option<TrackedCasDigest<Self>> {
        None
    }
}

/// Content identity of a canonical [`VersionSet`] CAS object.
pub type VersionSetDigest = CasDigest<VersionSetDigestKind>;

#[cfg(test)]
mod tests {
    use super::VersionSet;
    use crate::cas_digest::CasDigestConfig;
    use crate::cas_digest::DigestAlgorithm;

    /// Uses one stable algorithm so golden identities are platform-independent.
    fn digest_config() -> CasDigestConfig {
        CasDigestConfig::leak_new(vec![DigestAlgorithm::Sha256], None).unwrap()
    }

    #[test]
    fn invariant_version_set_has_versioned_canonical_identity() {
        let version_set = VersionSet::from_canonical_root(b"root");

        assert_eq!(version_set.as_bytes(), b"bsmr.version-set.v1\0root");
        assert_eq!(
            version_set.digest(digest_config()).to_string(),
            "ab6a68d13735d87c9b9f46f65b862b5ad538d6e8229393262c7ad4a688355b8d:24"
        );
    }

    #[test]
    fn invariant_semantic_mutation_changes_version_set_identity() {
        let original = VersionSet::from_canonical_root(b"package@1");
        let updated = VersionSet::from_canonical_root(b"package@2");

        assert_ne!(
            original.digest(digest_config()),
            updated.digest(digest_config())
        );
    }
}
