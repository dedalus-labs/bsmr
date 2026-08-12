//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Parses PEP 751 lock data without assuming responsibility for Python resolution.

use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;

use serde::Deserialize;

/// A parsed PEP 751 lock in deterministic wire order.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PylockToml {
    #[serde(deserialize_with = "deserialize_lock_version")]
    lock_version: String,
    pub created_by: String,
    pub requires_python: Option<String>,
    #[serde(default)]
    pub environments: Vec<String>,
    #[serde(default)]
    pub extras: Vec<String>,
    #[serde(default)]
    pub dependency_groups: Vec<String>,
    #[serde(default)]
    pub default_groups: Vec<String>,
    pub packages: Vec<PylockTomlPackage>,
    pub tool: Option<toml::Table>,
}

impl PylockToml {
    /// Parses and canonically orders a PEP 751 lock without resolving an environment.
    pub fn parse(input: &str) -> Result<Self, PylockTomlError> {
        let mut lock = toml::from_str::<Self>(input).map_err(PylockTomlError::Deserialize)?;
        lock.environments = sorted_unique(lock.environments);
        lock.extras = sorted_unique(lock.extras);
        lock.dependency_groups = sorted_unique(lock.dependency_groups);
        lock.default_groups = sorted_unique(lock.default_groups);
        lock.packages.iter_mut().for_each(PylockTomlPackage::sort);
        lock.packages.sort_by(|left, right| {
            (&left.name, &left.version, &left.marker).cmp(&(
                &right.name,
                &right.version,
                &right.marker,
            ))
        });
        Ok(lock)
    }

    /// Returns the accepted PEP 751 format version.
    pub fn lock_version(&self) -> &str {
        &self.lock_version
    }
}

/// One potentially marker-qualified package entry from a PEP 751 lock.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PylockTomlPackage {
    pub name: String,
    pub version: Option<String>,
    pub index: Option<String>,
    pub marker: Option<String>,
    pub requires_python: Option<String>,
    #[serde(default, rename = "dependencies")]
    _dependencies: Vec<toml::Table>,
    pub vcs: Option<toml::Table>,
    pub directory: Option<toml::Table>,
    pub archive: Option<PylockTomlArtifact>,
    pub sdist: Option<PylockTomlArtifact>,
    #[serde(default)]
    pub wheels: Vec<PylockTomlArtifact>,
    #[serde(default)]
    pub attestation_identities: Vec<toml::Table>,
    pub tool: Option<toml::Table>,
}

impl PylockTomlPackage {
    /// Canonically orders artifacts while preserving informational dependency data.
    fn sort(&mut self) {
        self.wheels.sort_by(|left, right| {
            (&left.name, &left.path, &left.url).cmp(&(&right.name, &right.path, &right.url))
        });
    }
}

/// Acquisition metadata for one locked Python distribution artifact.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PylockTomlArtifact {
    pub name: Option<String>,
    pub url: Option<String>,
    pub path: Option<String>,
    pub size: Option<u64>,
    pub upload_time: Option<toml::value::Datetime>,
    pub subdirectory: Option<String>,
    pub hashes: BTreeMap<String, String>,
}

/// The resolve identity encoded by a standard PEP 751 filename.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PylockName {
    Default,
    Named(String),
}

impl PylockName {
    /// Parses the exact standard filename and preserves a named resolve verbatim.
    pub fn from_path(path: &Path) -> Result<Self, PylockTomlError> {
        let Some(filename) = path.file_name().and_then(|filename| filename.to_str()) else {
            return Err(PylockTomlError::FileName(path.to_path_buf()));
        };
        if filename == "pylock.toml" {
            return Ok(Self::Default);
        }
        let Some(resolve) = filename
            .strip_prefix("pylock.")
            .and_then(|filename| filename.strip_suffix(".toml"))
            .filter(|resolve| !resolve.is_empty() && !resolve.contains('.'))
        else {
            return Err(PylockTomlError::FileName(path.to_path_buf()));
        };
        Ok(Self::Named(resolve.to_owned()))
    }

    /// Returns the named resolve, or `None` for the default lock.
    pub fn resolve(&self) -> Option<&str> {
        match self {
            Self::Default => None,
            Self::Named(resolve) => Some(resolve),
        }
    }
}

/// Failures that prevent a lock from entering Bessemer's Python frontend.
#[derive(Debug, bsmr_error::Error)]
#[bsmr(tag = Input)]
pub enum PylockTomlError {
    #[error("Invalid pylock.toml: {0}")]
    Deserialize(toml::de::Error),
    #[error(
        "Invalid PEP 751 lock filename `{}`; expected `pylock.toml` or `pylock.<name>.toml`",
        .0.display()
    )]
    FileName(PathBuf),
}

/// Accepts future minor revisions while rejecting unsupported lock major versions.
fn deserialize_lock_version<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let version = String::deserialize(deserializer)?;
    let mut release = version.split('.');
    let major = release.next().and_then(|value| value.parse().ok());
    let valid_minor = release.all(|value| value.parse::<u64>().is_ok());
    if major != Some(1_u64) || !valid_minor {
        return Err(serde::de::Error::custom(format_args!(
            "unsupported lock version `{version}`; only major version 1 is supported"
        )));
    }
    Ok(version)
}

/// Sorts set-like PEP 751 selection metadata for stable downstream identities.
fn sorted_unique(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use indoc::indoc;
    use test_case::test_case;

    use super::PylockName;
    use super::PylockToml;
    use super::PylockTomlError;

    const HEADER: &str = "lock-version = \"1.0\"\ncreated-by = \"uv\"\n";
    const ATTRS: &str = indoc! {r#"
        [[packages]]
        name = "attrs"
        version = "25.1.0"
        dependencies = [{ name = "typing-extensions" }]
        tool = { uv = { source = "registry" } }
        attestation-identities = [{ kind = "https://example.org/publisher" }]
        [[packages.wheels]]
        path = "z.whl"
        upload-time = 2025-01-25T11:30:10Z
        subdirectory = "dist"
        hashes = { sha256 = "z" }
        [[packages.wheels]]
        path = "a.whl"
        hashes = { sha256 = "a" }
    "#};
    const TYPING: &str = indoc! {r#"
        [[packages]]
        name = "typing-extensions"
        [[packages.wheels]]
        url = "https://example.org/typing.whl"
        hashes = { sha256 = "t" }
    "#};

    /// Package and artifact order must not leak into downstream action identities.
    #[test]
    fn invariant_wire_order_is_canonical() {
        let lock = PylockToml::parse(&format!("{HEADER}{TYPING}{ATTRS}")).unwrap();

        assert_eq!(lock.packages[0].name, "attrs");
        assert_eq!(lock.packages[0].wheels[0].path.as_deref(), Some("a.whl"));
        assert!(lock.packages[0].tool.is_some());
        assert_eq!(lock.packages[0].attestation_identities.len(), 1);
        let wheel = &lock.packages[0].wheels[1];
        assert!(wheel.upload_time.is_some());
        assert_eq!(wheel.subdirectory.as_deref(), Some("dist"));
    }

    /// Unknown minor revisions remain readable because extension keys are disposable.
    #[test]
    fn invariant_supported_major_accepts_future_minor() {
        let input = format!("{}future-key = true\n{ATTRS}", HEADER.replace("1.0", "1.1"));

        PylockToml::parse(&input).unwrap();
    }

    /// Invalid and unknown revisions must fail before semantic interpretation.
    #[test_case("1.next")]
    #[test_case("2.0")]
    fn invariant_unsupported_version_is_rejected(version: &str) {
        let input = format!("{}{ATTRS}", HEADER.replace("1.0", version));

        assert!(matches!(
            PylockToml::parse(&input),
            Err(PylockTomlError::Deserialize(_))
        ));
    }

    /// Standard and named lock filenames must map to unambiguous resolve identities.
    #[test_case("pylock.toml", None)]
    #[test_case("pylock.ci.toml", Some("ci"))]
    fn invariant_lock_filename_maps_to_one_resolve(path: &str, expected: Option<&str>) {
        let name = PylockName::from_path(Path::new(path)).unwrap();

        assert_eq!(name.resolve(), expected);
    }

    /// Non-standard filenames must not silently become Bessemer resolves.
    #[test_case("requirements.txt"; "requirements")]
    #[test_case("Pylock.toml"; "uppercase_prefix")]
    #[test_case("pylock..toml"; "empty_name")]
    #[test_case("pylock.dev.test.toml"; "dotted_name")]
    fn invariant_nonstandard_lock_filename_is_rejected(path: &str) {
        assert!(matches!(
            PylockName::from_path(Path::new(path)),
            Err(PylockTomlError::FileName(_))
        ));
    }
}
