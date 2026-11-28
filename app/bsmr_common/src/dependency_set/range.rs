//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Models structured integer contract ranges and evaluates intersections via astral-version-ranges.

use std::collections::BTreeSet;
use std::fmt;

use serde::Deserialize;
use serde::Serialize;
use version_ranges::Ranges;

use super::DependencySetError;

/// Structured integer range over a named interface contract version.
///
/// Deserialized directly by `serde` without any custom string grammar:
/// - Inclusive bounds: `{ min = 2, max = 4 }`, `{ min = 7 }`, or `{ max = 8 }`
/// - Explicit version set: `{ versions = [1, 3] }`
///
/// Contiguous explicit sets canonicalize onto `{ min, max }` so equivalent
/// declarations hash to identical digests.
#[derive(Clone, Debug, Default, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ContractRange {
    /// Inclusive lower bound (`>= min`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min: Option<u64>,
    /// Inclusive upper bound (`<= max`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max: Option<u64>,
    /// Explicit non-empty set of allowed integer versions.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub versions: Option<Vec<u64>>,
}

/// Normalized discrete `u64` interval set backed by `astral-version-ranges`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractRangeSet {
    ranges: Ranges<u64>,
}

impl ContractRange {
    /// Constructs an inclusive bounded range (`min <= version <= max`).
    #[must_use]
    pub fn bounds(min: Option<u64>, max: Option<u64>) -> Self {
        Self {
            min,
            max,
            versions: None,
        }
    }

    /// Constructs a range permitting a single exact integer contract version.
    #[must_use]
    pub fn exact(version: u64) -> Self {
        Self::bounds(Some(version), Some(version))
    }

    /// Constructs a range permitting an explicit list of integer versions.
    #[must_use]
    pub fn versions(versions: Vec<u64>) -> Self {
        Self {
            min: None,
            max: None,
            versions: Some(versions),
        }
    }

    /// Validates invariants and canonicalizes equivalent representations in place.
    pub fn validate_and_canonicalize(&mut self, contract: &str) -> Result<(), DependencySetError> {
        let has_bounds = self.min.is_some() || self.max.is_some();
        if let Some(raw_versions) = &self.versions {
            if has_bounds {
                return Err(invalid_range(
                    contract,
                    "cannot combine `min`/`max` bounds with an explicit `versions` list",
                ));
            }
            if raw_versions.is_empty() {
                return Err(invalid_range(contract, "`versions` list must not be empty"));
            }
            let unique: Vec<u64> = raw_versions
                .iter()
                .copied()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
            let (first, last) = (unique[0], unique[unique.len() - 1]);
            let contiguous = last
                .checked_sub(first)
                .and_then(|span| usize::try_from(span).ok())
                .is_some_and(|span| span + 1 == unique.len());
            if contiguous {
                self.min = Some(first);
                self.max = Some(last);
                self.versions = None;
            } else {
                self.versions = Some(unique);
            }
            return Ok(());
        }
        if !has_bounds {
            return Err(invalid_range(
                contract,
                "range must specify `min`, `max`, or `versions`",
            ));
        }
        if let (Some(min), Some(max)) = (self.min, self.max) {
            if min > max {
                return Err(invalid_range(
                    contract,
                    format!("`min` ({min}) must not exceed `max` ({max})"),
                ));
            }
        }
        Ok(())
    }

    /// Converts this validated range into an [`astral-version-ranges`](Ranges) interval set.
    pub fn to_range_set(&self, contract: &str) -> Result<ContractRangeSet, DependencySetError> {
        let mut canonical = self.clone();
        canonical.validate_and_canonicalize(contract)?;
        let ranges = if let Some(versions) = canonical.versions {
            versions
                .into_iter()
                .fold(Ranges::empty(), |acc, v| acc.union(&Ranges::singleton(v)))
        } else {
            match (canonical.min, canonical.max) {
                (Some(min), Some(max)) => Ranges::from_range_bounds(min..=max),
                (Some(min), None) => Ranges::higher_than(min),
                (None, Some(max)) => Ranges::from_range_bounds(..=max),
                (None, None) => Ranges::full(),
            }
        };
        Ok(ContractRangeSet { ranges })
    }
}

impl fmt::Display for ContractRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(versions) = &self.versions {
            let joined = versions
                .iter()
                .map(u64::to_string)
                .collect::<Vec<_>>()
                .join(", ");
            return write!(f, "{{ versions = [{joined}] }}");
        }
        match (self.min, self.max) {
            (Some(min), Some(max)) => write!(f, "{{ min = {min}, max = {max} }}"),
            (Some(min), None) => write!(f, "{{ min = {min} }}"),
            (None, Some(max)) => write!(f, "{{ max = {max} }}"),
            (None, None) => f.write_str("{}"),
        }
    }
}

impl ContractRangeSet {
    /// Returns the intersection of two discrete contract version sets.
    #[must_use]
    pub fn intersection(&self, other: &Self) -> Self {
        Self {
            ranges: self.ranges.intersection(&other.ranges),
        }
    }

    /// Returns true if `version` belongs to this range set.
    #[must_use]
    pub fn contains(&self, version: u64) -> bool {
        self.ranges.contains(&version)
    }

    /// Returns true if no integer version satisfies this range set.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.ranges.is_empty()
    }
}

impl fmt::Display for ContractRangeSet {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.ranges.fmt(f)
    }
}

/// Constructs a [`DependencySetError::InvalidContractRange`] error.
fn invalid_range(contract: &str, reason: impl Into<String>) -> DependencySetError {
    DependencySetError::InvalidContractRange {
        contract: contract.to_owned(),
        reason: reason.into(),
    }
}
