//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Parses PEP 751 lock data without assuming responsibility for Python resolution.

use std::collections::BTreeMap;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::LazyLock;

use serde::Deserialize;
use uv_distribution_filename::SourceDistExtension;
use uv_distribution_filename::SourceDistFilename;
use uv_distribution_filename::WheelFilename;
use uv_pep508::MarkerTree;
use uv_platform_tags::Arch;
use uv_platform_tags::Os;
use uv_platform_tags::Platform;
use uv_platform_tags::Tags;
use uv_platform_tags::TagsOptions;

/// Exact environment facts shared by every platform in the native Python catalog.
static SUPPORTED_PYTHON_DOMAIN: LazyLock<MarkerTree> = LazyLock::new(|| {
    MarkerTree::from_str(concat!(
        "implementation_name == 'cpython' and ",
        "platform_python_implementation == 'CPython' and ",
        "os_name == 'posix' and (",
        "(sys_platform == 'darwin' and platform_system == 'Darwin' and ",
        "(platform_machine == 'arm64' or platform_machine == 'x86_64')) or ",
        "(sys_platform == 'linux' and platform_system == 'Linux' and ",
        "(platform_machine == 'aarch64' or platform_machine == 'x86_64'))",
        ")",
    ))
    .expect("the native Python toolchain domain is a valid PEP 508 marker")
});

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
    #[serde(skip)]
    raw: toml::Table,
}

/// One self-contained PEP 751 input for a normalized package action.
#[derive(Debug, Eq, PartialEq)]
pub struct PylockInstallationFragment {
    pub package: String,
    pub contents: String,
    pub acquisition: PylockAcquisition,
    /// One unconditionally compatible wheel that bypasses selection.
    pub artifact: Option<PylockArtifact>,
    /// One immutable source archive acquired before a PEP 517 action.
    pub source_artifact: Option<PylockSourceArtifact>,
    /// One immutable VCS tree acquired before a PEP 517 action.
    pub vcs_source: Option<PylockVcsSource>,
    /// One local source tree represented by a declared first-party target.
    pub directory_source: Option<PylockDirectorySource>,
    /// Exact wheel candidates keyed by Python line and execution platform.
    pub artifacts: BTreeMap<String, Vec<PylockArtifact>>,
}

/// One locked distribution artifact BSMR can acquire without index resolution.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PylockArtifact {
    pub filename: String,
    pub sha256: String,
    pub size: u64,
    pub url: String,
}

/// One source artifact plus the locked build identity it must produce.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PylockSourceArtifact {
    /// Digest-verified archive acquired independently of the build action.
    pub artifact: PylockArtifact,
    /// Optional normalized project root within a nonstandard archive.
    pub subdirectory: Option<String>,
    /// Exact distribution version the source build must produce.
    pub version: String,
}

/// One immutable VCS tree plus the locked build identity it must produce.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PylockVcsSource {
    /// Full immutable Git object identity selected by the lock.
    pub commit: String,
    /// Optional normalized project root within the checkout.
    pub subdirectory: Option<String>,
    /// Credential-free HTTPS repository address.
    pub url: String,
    /// Exact distribution version the source build must produce.
    pub version: String,
}

/// One local source tree that must map to a declared workspace project.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PylockDirectorySource {
    /// Whether the authoring environment requested editable installation semantics.
    pub editable: bool,
    /// Normalized project root relative to the lock file.
    pub path: String,
}

/// The artifact forms available to uv for one locked distribution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PylockAcquisition {
    Wheel,
    Source,
    WheelOrSource,
}

impl PylockAcquisition {
    /// Returns whether the selected distribution may need a PEP 517 build.
    pub fn permits_source(self) -> bool {
        matches!(self, Self::Source | Self::WheelOrSource)
    }

    /// Returns the stable Starlark spelling consumed by the native rule.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Wheel => "wheel",
            Self::Source => "source",
            Self::WheelOrSource => "wheel-or-source",
        }
    }
}

impl PylockToml {
    /// Parses and canonically orders a PEP 751 lock without resolving an environment.
    pub fn parse(input: &str) -> Result<Self, PylockTomlError> {
        let mut lock = toml::from_str::<Self>(input).map_err(PylockTomlError::Deserialize)?;
        lock.raw = toml::from_str(input).map_err(PylockTomlError::Deserialize)?;
        lock.validate()?;
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

    /// Splits the lock into canonical per-package installation inputs.
    pub fn installation_fragments(
        &self,
    ) -> Result<Vec<PylockInstallationFragment>, PylockTomlError> {
        let mut base = self.raw.clone();
        let packages = base
            .remove("packages")
            .and_then(|packages| packages.as_array().cloned())
            .expect("typed PEP 751 parsing requires a package array");
        canonicalize_selection_metadata(&mut base, self);
        canonicalize_table(&mut base);
        let mut groups = BTreeMap::<String, (bool, bool, bool, Vec<toml::Value>)>::new();
        for package in packages {
            let mut package = package
                .as_table()
                .cloned()
                .expect("typed PEP 751 parsing requires package tables");
            let name = package
                .get("name")
                .and_then(toml::Value::as_str)
                .expect("typed PEP 751 parsing requires package names")
                .to_owned();
            let (has_wheel, has_source) = package_artifact_forms(&package);
            let has_universal_wheel = package_has_universal_wheel(&package);
            package.remove("dependencies");
            canonicalize_table(&mut package);
            let group = groups
                .entry(name)
                .or_insert_with(|| (false, false, true, Vec::new()));
            group.0 |= has_wheel;
            group.1 |= has_source;
            group.2 &= has_universal_wheel;
            group.3.push(toml::Value::Table(package));
        }
        groups
            .into_iter()
            .map(
                |(package, (has_wheel, has_source, universal, mut variants))| {
                    variants.sort_by_cached_key(toml::Value::to_string);
                    let artifact = match variants.as_slice() {
                        [variant] => direct_universal_wheel(variant),
                        _ => None,
                    };
                    let artifacts = if artifact.is_some() {
                        BTreeMap::new()
                    } else {
                        platform_wheels(&package, &variants)?
                    };
                    let source_artifact = if has_source && !universal {
                        match variants.as_slice() {
                            [variant] => direct_source_artifact(variant),
                            _ => None,
                        }
                    } else {
                        None
                    };
                    let vcs_source = if has_source && !universal {
                        match variants.as_slice() {
                            [variant] => direct_vcs_source(variant),
                            _ => None,
                        }
                    } else {
                        None
                    };
                    let directory_source = if has_source && !universal {
                        match variants.as_slice() {
                            [variant] => direct_directory_source(variant),
                            _ => None,
                        }
                    } else {
                        None
                    };
                    if directory_source.is_none()
                        && variants.iter().any(|variant| {
                            variant
                                .as_table()
                                .is_some_and(|variant| variant.contains_key("directory"))
                        })
                    {
                        return Err(PylockTomlError::Package {
                            package,
                            reason: "directory sources require one unconditional variant",
                        });
                    }
                    let mut fragment = base.clone();
                    fragment.insert("packages".to_owned(), toml::Value::Array(variants));
                    Ok(PylockInstallationFragment {
                        package,
                        contents: toml::to_string(&fragment).map_err(PylockTomlError::Serialize)?,
                        acquisition: match (has_wheel, has_source, universal) {
                            (true, _, true) => PylockAcquisition::Wheel,
                            (true, true, false) => PylockAcquisition::WheelOrSource,
                            (true, false, false) => PylockAcquisition::Wheel,
                            (false, true, false) => PylockAcquisition::Source,
                            (false, true, true) => {
                                unreachable!("a universal wheel is a wheel artifact")
                            }
                            (false, false, _) => {
                                unreachable!("a package always has one artifact form")
                            }
                        },
                        artifact,
                        source_artifact,
                        vcs_source,
                        directory_source,
                        artifacts,
                    })
                },
            )
            .collect()
    }

    /// Rejects lock data that cannot identify one reproducible installation.
    fn validate(&self) -> Result<(), PylockTomlError> {
        self.packages
            .iter()
            .try_for_each(PylockTomlPackage::validate)
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
    pub vcs: Option<PylockTomlVcs>,
    pub directory: Option<PylockTomlDirectory>,
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

    /// Validates one package's normalized identity and acquisition source.
    fn validate(&self) -> Result<(), PylockTomlError> {
        if normalize_package_name(&self.name).as_deref() != Some(&self.name) {
            return Err(self.invalid("name must be normalized"));
        }
        if self
            .marker
            .as_deref()
            .is_some_and(|marker| MarkerTree::from_str(marker).is_err())
        {
            return Err(self.invalid("marker must be a valid PEP 508 expression"));
        }
        let source_count = usize::from(self.vcs.is_some())
            + usize::from(self.directory.is_some())
            + usize::from(self.archive.is_some())
            + usize::from(self.sdist.is_some() || !self.wheels.is_empty());
        if source_count == 0 {
            return Err(self.invalid("acquisition source is required"));
        }
        if source_count > 1 {
            return Err(self.invalid("acquisition sources are mutually exclusive"));
        }
        if let Some(vcs) = &self.vcs {
            vcs.validate(self)?;
        }
        if let Some(directory) = &self.directory {
            if directory.path.is_empty() {
                return Err(self.invalid("directory path must not be empty"));
            }
            if !normalized_directory_path(&directory.path) {
                return Err(self.invalid("directory path must be a normalized relative path"));
            }
            if directory
                .subdirectory
                .as_deref()
                .is_some_and(|path| !normalized_subdirectory(path))
            {
                return Err(
                    self.invalid("directory subdirectory must be a normalized relative path")
                );
            }
        }
        for (kind, artifact) in self.artifacts() {
            artifact.validate(self, kind)?;
        }
        if self.attestation_identities.iter().any(|identity| {
            identity
                .get("kind")
                .and_then(toml::Value::as_str)
                .is_none_or(str::is_empty)
        }) {
            return Err(self.invalid("attestation identity kind is required"));
        }
        Ok(())
    }

    /// Iterates over every artifact form with its PEP 751 field name.
    fn artifacts(&self) -> impl Iterator<Item = (&'static str, &PylockTomlArtifact)> {
        self.archive
            .iter()
            .map(|artifact| ("archive", artifact))
            .chain(self.sdist.iter().map(|artifact| ("sdist", artifact)))
            .chain(self.wheels.iter().map(|artifact| ("wheel", artifact)))
    }

    /// Associates a semantic lock failure with the offending package.
    fn invalid(&self, reason: &'static str) -> PylockTomlError {
        PylockTomlError::Package {
            package: self.name.clone(),
            reason,
        }
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

impl PylockTomlArtifact {
    /// Requires one address and one nonempty immutable digest.
    fn validate(
        &self,
        package: &PylockTomlPackage,
        kind: &'static str,
    ) -> Result<(), PylockTomlError> {
        let located = self.url.as_deref().is_some_and(|url| !url.is_empty())
            || self.path.as_deref().is_some_and(|path| !path.is_empty());
        if !located {
            return Err(package.invalid(match kind {
                "archive" => "archive URL or path is required",
                "sdist" => "sdist URL or path is required",
                "wheel" => "wheel URL or path is required",
                _ => unreachable!("artifact kinds are finite"),
            }));
        }
        if self.hashes.is_empty()
            || self
                .hashes
                .iter()
                .any(|(algorithm, digest)| algorithm.is_empty() || digest.is_empty())
        {
            return Err(package.invalid(match kind {
                "archive" => "archive hashes must not be empty",
                "sdist" => "sdist hashes must not be empty",
                "wheel" => "wheel hashes must not be empty",
                _ => unreachable!("artifact kinds are finite"),
            }));
        }
        if self
            .subdirectory
            .as_deref()
            .is_some_and(|path| !normalized_subdirectory(path))
        {
            return Err(package.invalid("artifact subdirectory must be a normalized relative path"));
        }
        Ok(())
    }
}

/// An immutable VCS source selected by its resolved commit identity.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct PylockTomlVcs {
    #[serde(rename = "type")]
    pub kind: String,
    pub url: Option<String>,
    pub path: Option<String>,
    pub requested_revision: Option<String>,
    pub commit_id: String,
    pub subdirectory: Option<String>,
}

impl PylockTomlVcs {
    /// Requires an immutable revision and an address for its source tree.
    fn validate(&self, package: &PylockTomlPackage) -> Result<(), PylockTomlError> {
        if self.kind.is_empty() {
            return Err(package.invalid("VCS type must not be empty"));
        }
        if self.commit_id.is_empty() {
            return Err(package.invalid("VCS commit ID must not be empty"));
        }
        let located = self.url.as_deref().is_some_and(|url| !url.is_empty())
            || self.path.as_deref().is_some_and(|path| !path.is_empty());
        if !located {
            return Err(package.invalid("VCS URL or path is required"));
        }
        if self
            .subdirectory
            .as_deref()
            .is_some_and(|path| !normalized_subdirectory(path))
        {
            return Err(package.invalid("VCS subdirectory must be a normalized relative path"));
        }
        Ok(())
    }
}

/// A local Python source tree referenced relative to its lockfile.
#[derive(Debug, Deserialize)]
pub struct PylockTomlDirectory {
    pub path: String,
    pub editable: Option<bool>,
    pub subdirectory: Option<String>,
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
    #[error("Unable to serialize canonical pylock.toml fragment: {0}")]
    Serialize(toml::ser::Error),
    #[error(
        "Invalid PEP 751 lock filename `{}`; expected `pylock.toml` or `pylock.<name>.toml`",
        .0.display()
    )]
    FileName(PathBuf),
    #[error("Invalid pylock.toml package `{package}`: {reason}")]
    Package {
        package: String,
        reason: &'static str,
    },
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

/// Rewrites set-like selection arrays to their validated canonical order.
fn canonicalize_selection_metadata(table: &mut toml::Table, lock: &PylockToml) {
    for (name, values) in [
        ("environments", &lock.environments),
        ("extras", &lock.extras),
        ("dependency-groups", &lock.dependency_groups),
        ("default-groups", &lock.default_groups),
    ] {
        if table.contains_key(name) || !values.is_empty() {
            table.insert(
                name.to_owned(),
                toml::Value::Array(values.iter().cloned().map(toml::Value::String).collect()),
            );
        }
    }
}

/// Returns whether a package offers wheel and source installation forms.
fn package_artifact_forms(package: &toml::Table) -> (bool, bool) {
    let has_wheel = package
        .get("wheels")
        .and_then(toml::Value::as_array)
        .is_some_and(|wheels| !wheels.is_empty());
    let has_source = package.contains_key("vcs")
        || package.contains_key("directory")
        || package.contains_key("archive")
        || package.contains_key("sdist")
        || !has_wheel;
    (has_wheel, has_source)
}

/// Returns whether a package offers a platform-independent Python 3 wheel.
fn package_has_universal_wheel(package: &toml::Table) -> bool {
    package
        .get("wheels")
        .and_then(toml::Value::as_array)
        .is_some_and(|wheels| wheels.iter().any(universal_wheel))
}

/// Recognizes wheel tags that every supported CPython 3 toolchain accepts.
fn universal_wheel(wheel: &toml::Value) -> bool {
    let Some(wheel) = wheel.as_table() else {
        return false;
    };
    ["name", "path", "url"]
        .into_iter()
        .filter_map(|field| wheel.get(field).and_then(toml::Value::as_str))
        .map(|location| location.split(['?', '#']).next().unwrap_or(location))
        .any(|location| {
            location.ends_with("-py3-none-any.whl") || location.ends_with("-py2.py3-none-any.whl")
        })
}

/// Partitions exact wheel candidates by BSMR's finite execution-platform catalog.
fn platform_wheels(
    package: &str,
    variants: &[toml::Value],
) -> Result<BTreeMap<String, Vec<PylockArtifact>>, PylockTomlError> {
    let tags = native_python_tags();
    let mut artifacts = tags
        .iter()
        .map(|(platform, _)| (platform.clone(), BTreeMap::<String, PylockArtifact>::new()))
        .collect::<BTreeMap<_, _>>();
    for variant in variants {
        let Some(variant) = variant.as_table() else {
            return Ok(BTreeMap::new());
        };
        let Some(version) = variant.get("version").and_then(toml::Value::as_str) else {
            return Ok(BTreeMap::new());
        };
        let Some(wheels) = variant.get("wheels").and_then(toml::Value::as_array) else {
            continue;
        };
        for wheel in wheels {
            let Some(filename) = locked_artifact_filename(wheel) else {
                return Ok(BTreeMap::new());
            };
            let Ok(parsed) = WheelFilename::from_str(filename) else {
                return Ok(BTreeMap::new());
            };
            let compatible = tags
                .iter()
                .filter_map(|(platform, tags)| {
                    parsed.is_compatible(tags).then_some(platform.as_str())
                })
                .collect::<Vec<_>>();
            if compatible.is_empty() {
                continue;
            }
            let Some(artifact) = downloadable_wheel(package, version, filename, wheel) else {
                return Ok(BTreeMap::new());
            };
            for platform in compatible {
                let selected = artifacts
                    .get_mut(platform)
                    .expect("native platform keys originate from the same catalog");
                if selected
                    .insert(filename.to_owned(), artifact.clone())
                    .is_some_and(|existing| existing != artifact)
                {
                    return Err(PylockTomlError::Package {
                        package: package.to_owned(),
                        reason: "wheel filename identifies multiple artifacts",
                    });
                }
            }
        }
    }
    if artifacts.values().all(BTreeMap::is_empty) {
        return Ok(BTreeMap::new());
    }
    Ok(artifacts
        .into_iter()
        .map(|(platform, artifacts)| (platform, artifacts.into_values().collect()))
        .collect())
}

/// Generates the same target-tag domains passed to uv by the native toolchain.
fn native_python_tags() -> Vec<(String, Tags)> {
    let platforms = [
        (
            "linux-arm64",
            Platform::new(
                Os::Manylinux {
                    major: 2,
                    minor: 28,
                },
                Arch::Aarch64,
            ),
        ),
        (
            "linux-x86_64",
            Platform::new(
                Os::Manylinux {
                    major: 2,
                    minor: 28,
                },
                Arch::X86_64,
            ),
        ),
        (
            "macos-arm64",
            Platform::new(
                Os::Macos {
                    major: 13,
                    minor: 0,
                },
                Arch::Aarch64,
            ),
        ),
        (
            "macos-x86_64",
            Platform::new(
                Os::Macos {
                    major: 13,
                    minor: 0,
                },
                Arch::X86_64,
            ),
        ),
    ];
    platforms
        .into_iter()
        .flat_map(|(name, platform)| {
            let manylinux_compatible = matches!(platform.os(), Os::Manylinux { .. });
            [(3, 13), (3, 14)].into_iter().map(move |version| {
                (
                    format!("{}.{}-{name}", version.0, version.1),
                    Tags::from_env(
                        platform.clone(),
                        version,
                        "cpython",
                        version,
                        TagsOptions {
                            manylinux_compatible,
                            is_cross: true,
                            ..TagsOptions::default()
                        },
                    )
                    .expect("native Python catalog entries have valid wheel tags"),
                )
            })
        })
        .collect()
}

/// Returns the declared artifact filename without trusting URL query data.
fn locked_artifact_filename(artifact: &toml::Value) -> Option<&str> {
    let artifact = artifact.as_table()?;
    ["name", "path", "url"]
        .into_iter()
        .find_map(|field| artifact.get(field).and_then(toml::Value::as_str))?
        .split(['?', '#'])
        .next()?
        .rsplit('/')
        .next()
}

/// Converts one locked remote artifact into an authenticated acquisition input.
fn downloadable_artifact(filename: &str, artifact: &toml::Table) -> Option<PylockArtifact> {
    let url = artifact.get("url")?.as_str()?;
    let location = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let (authority, path) = location.split_once('/')?;
    if authority.is_empty()
        || path.is_empty()
        || authority.contains(['@', '%', '\\'])
        || !url.bytes().all(|byte| byte.is_ascii_graphic())
        || url.contains('\\')
        || url.contains(['?', '#'])
        || !filename.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'+' | b'!')
        })
    {
        return None;
    }
    let hashes = artifact.get("hashes")?.as_table()?;
    let sha256 = hashes.get("sha256")?.as_str()?;
    if sha256.len() != 64 || !sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let size = artifact
        .get("size")
        .and_then(toml::Value::as_integer)
        .and_then(|size| u64::try_from(size).ok())?;
    Some(PylockArtifact {
        filename: filename.to_owned(),
        sha256: sha256.to_owned(),
        size,
        url: url.to_owned(),
    })
}

/// Converts one compatible wheel into an independently authenticated download.
fn downloadable_wheel(
    package: &str,
    version: &str,
    filename: &str,
    wheel: &toml::Value,
) -> Option<PylockArtifact> {
    let parsed = WheelFilename::from_str(filename).ok()?;
    if parsed.name.to_string() != package || parsed.version.to_string() != version {
        return None;
    }
    let wheel = wheel.as_table()?;
    downloadable_artifact(filename, wheel)
}

/// Extracts one directly downloadable source archive from an unconditional variant.
fn direct_source_artifact(package: &toml::Value) -> Option<PylockSourceArtifact> {
    let package = package.as_table()?;
    if package.contains_key("requires-python")
        || package
            .get("marker")
            .and_then(toml::Value::as_str)
            .is_some_and(|marker| !marker_covers_supported_python(marker))
    {
        return None;
    }
    let (source, standard_sdist) = package
        .get("sdist")
        .map(|source| (source, true))
        .or_else(|| package.get("archive").map(|source| (source, false)))?;
    let subdirectory = source
        .get("subdirectory")
        .and_then(toml::Value::as_str)
        .map(str::to_owned);
    let filename = locked_artifact_filename(source)?;
    let package_name = package.get("name")?.as_str()?;
    let version = package.get("version")?.as_str()?;
    if standard_sdist {
        let parsed = SourceDistFilename::parsed_normalized_filename(filename).ok()?;
        if subdirectory.is_some()
            || parsed.name.to_string() != package_name
            || parsed.version.to_string() != version
        {
            return None;
        }
    } else {
        SourceDistExtension::from_path(filename).ok()?;
    }
    Some(PylockSourceArtifact {
        artifact: downloadable_artifact(filename, source.as_table()?)?,
        subdirectory,
        version: version.to_owned(),
    })
}

/// Extracts one immutable Git tree from an unconditional lock variant.
fn direct_vcs_source(package: &toml::Value) -> Option<PylockVcsSource> {
    let package = package.as_table()?;
    if package.contains_key("requires-python")
        || package
            .get("marker")
            .and_then(toml::Value::as_str)
            .is_some_and(|marker| !marker_covers_supported_python(marker))
    {
        return None;
    }
    let vcs = package.get("vcs")?.as_table()?;
    if vcs.get("type").and_then(toml::Value::as_str) != Some("git") || vcs.contains_key("path") {
        return None;
    }
    let url = vcs.get("url")?.as_str()?;
    let location = url.strip_prefix("https://")?;
    let (authority, path) = location.split_once('/')?;
    let commit = vcs.get("commit-id")?.as_str()?;
    if authority.is_empty()
        || path.is_empty()
        || authority.contains(['@', '%', '\\'])
        || !url.bytes().all(|byte| byte.is_ascii_graphic())
        || url.contains('\\')
        || url.contains(['?', '#'])
        || !matches!(commit.len(), 40 | 64)
        || !commit
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return None;
    }
    Some(PylockVcsSource {
        commit: commit.to_owned(),
        subdirectory: vcs
            .get("subdirectory")
            .and_then(toml::Value::as_str)
            .map(str::to_owned),
        url: url.to_owned(),
        version: package.get("version")?.as_str()?.to_owned(),
    })
}

/// Extracts one local project root from an unconditional lock variant.
fn direct_directory_source(package: &toml::Value) -> Option<PylockDirectorySource> {
    let package = package.as_table()?;
    if package.contains_key("requires-python")
        || package
            .get("marker")
            .and_then(toml::Value::as_str)
            .is_some_and(|marker| !marker_covers_supported_python(marker))
    {
        return None;
    }
    let directory = package.get("directory")?.as_table()?;
    let path = directory.get("path")?.as_str()?;
    let subdirectory = directory.get("subdirectory").and_then(toml::Value::as_str);
    let path = match (path, subdirectory) {
        (".", Some(subdirectory)) => subdirectory.to_owned(),
        (".", None) => String::new(),
        (path, Some(subdirectory)) => format!("{path}/{subdirectory}"),
        (path, None) => path.to_owned(),
    };
    Some(PylockDirectorySource {
        editable: directory
            .get("editable")
            .and_then(toml::Value::as_bool)
            .unwrap_or(false),
        path,
    })
}

/// Extracts one directly downloadable universal wheel from a single lock variant.
fn direct_universal_wheel(package: &toml::Value) -> Option<PylockArtifact> {
    let package = package.as_table()?;
    if package.contains_key("requires-python")
        || package
            .get("marker")
            .and_then(toml::Value::as_str)
            .is_some_and(|marker| !marker_covers_supported_python(marker))
    {
        return None;
    }
    let wheels = package.get("wheels")?.as_array()?;
    let [wheel] = wheels.as_slice() else {
        return None;
    };
    if !universal_wheel(wheel) {
        return None;
    }
    let wheel = wheel.as_table()?;
    let url = wheel.get("url")?.as_str()?;
    let location = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let (authority, path) = location.split_once('/')?;
    let filename = path.rsplit('/').next()?;
    let identity = filename
        .strip_suffix("-py3-none-any.whl")
        .or_else(|| filename.strip_suffix("-py2.py3-none-any.whl"))?;
    let (wheel_name, wheel_version) = identity.split_once('-')?;
    if authority.is_empty()
        || authority.contains(['@', '%', '\\'])
        || !authority.bytes().all(|byte| byte.is_ascii_graphic())
        || url.contains(['?', '#'])
        || !filename.bytes().all(|byte| {
            byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-' | b'+' | b'!')
        })
        || wheel_version.contains('-')
        || normalize_package_name(wheel_name).as_deref()
            != package.get("name").and_then(toml::Value::as_str)
        || Some(wheel_version) != package.get("version").and_then(toml::Value::as_str)
    {
        return None;
    }
    downloadable_artifact(filename, wheel)
}

/// Proves that a package marker selects every native BSMR Python platform.
fn marker_covers_supported_python(marker: &str) -> bool {
    MarkerTree::from_str(marker)
        .is_ok_and(|marker| SUPPORTED_PYTHON_DOMAIN.implies(marker).is_true())
}

/// Recursively sorts TOML table keys without reordering semantic arrays.
fn canonicalize_table(table: &mut toml::Table) {
    let mut entries = std::mem::take(table).into_iter().collect::<Vec<_>>();
    entries.sort_by(|(left, _), (right, _)| left.cmp(right));
    for (name, mut value) in entries {
        canonicalize_value(&mut value);
        table.insert(name, value);
    }
}

/// Canonicalizes nested extension tables while preserving their array order.
fn canonicalize_value(value: &mut toml::Value) {
    match value {
        toml::Value::Table(table) => canonicalize_table(table),
        toml::Value::Array(values) => values.iter_mut().for_each(canonicalize_value),
        _ => {}
    }
}

/// Applies the Python distribution-name normalization algorithm.
fn normalize_package_name(name: &str) -> Option<String> {
    if name.is_empty()
        || name
            .as_bytes()
            .first()
            .is_none_or(|character| !character.is_ascii_alphanumeric())
        || name
            .as_bytes()
            .last()
            .is_none_or(|character| !character.is_ascii_alphanumeric())
    {
        return None;
    }
    let mut normalized = String::with_capacity(name.len());
    let mut separator = false;
    for character in name.chars() {
        if character.is_ascii_alphanumeric() {
            normalized.push(character.to_ascii_lowercase());
            separator = false;
        } else if matches!(character, '-' | '_' | '.') {
            if !separator {
                normalized.push('-');
                separator = true;
            }
        } else {
            return None;
        }
    }
    Some(normalized)
}

/// Accepts only portable non-root paths within an acquired source tree.
fn normalized_subdirectory(path: &str) -> bool {
    !path.is_empty()
        && !path.starts_with('/')
        && !path.contains('\\')
        && !(path.len() > 1 && path.as_bytes()[1] == b':')
        && path
            .split('/')
            .all(|component| !component.is_empty() && component != "." && component != "..")
}

/// Accepts the lock directory itself or one portable child path.
fn normalized_directory_path(path: &str) -> bool {
    path == "." || normalized_subdirectory(path)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use indoc::indoc;
    use test_case::test_case;

    use super::PylockAcquisition;
    use super::PylockArtifact;
    use super::PylockDirectorySource;
    use super::PylockInstallationFragment;
    use super::PylockName;
    use super::PylockSourceArtifact;
    use super::PylockToml;
    use super::PylockTomlError;

    const HEADER: &str = "lock-version = \"1.0\"\ncreated-by = \"uv\"\n";
    const SHA256: &str = "0000000000000000000000000000000000000000000000000000000000000000";
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

    /// Asserts one semantic package failure without accepting a parser error.
    fn assert_package_error(input: &str, expected_package: &str, expected: &str) {
        match PylockToml::parse(input) {
            Err(PylockTomlError::Package { package, reason }) => {
                assert_eq!(package, expected_package);
                assert_eq!(reason, expected);
            }
            result => panic!("expected package error, got {result:?}"),
        }
    }

    /// Returns one package fragment from concise direct-wheel fixture fields.
    fn direct_fragment(package_fields: &str, wheels: &str) -> PylockInstallationFragment {
        let input =
            format!("{HEADER}[[packages]]\nname = 'demo'\nversion = '1'\n{package_fields}{wheels}");
        PylockToml::parse(&input)
            .unwrap()
            .installation_fragments()
            .unwrap()
            .pop()
            .unwrap()
    }

    /// Returns one complete size- and SHA-256-pinned wheel table.
    fn locked_wheel(url: &str) -> String {
        format!(
            "[[packages.wheels]]\nurl = {url:?}\nsize = 42\nhashes = {{ sha256 = {SHA256:?} }}\n"
        )
    }

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

    /// Immutable VCS identity must survive the wire boundary as typed data.
    #[test]
    fn invariant_vcs_commit_and_stable_version_are_preserved() {
        let commit = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeef";
        let input = format!(
            "{HEADER}[[packages]]\nname = \"demo\"\nversion = \"1.0\"\n[packages.vcs]\ntype = \"git\"\nurl = \"https://example.org/demo.git\"\ncommit-id = \"{commit}\"\n"
        );
        let lock = PylockToml::parse(&input).unwrap();
        let vcs = lock.packages[0].vcs.as_ref().unwrap();

        assert_eq!(lock.packages[0].version.as_deref(), Some("1.0"));
        assert_eq!(vcs.kind, "git");
        assert_eq!(vcs.commit_id, commit);
        let fragment = &lock.installation_fragments().unwrap()[0];
        assert_eq!(fragment.acquisition, PylockAcquisition::Source);
        let source = fragment.vcs_source.as_ref().unwrap();
        assert_eq!(source.commit, commit);
        assert_eq!(source.url, "https://example.org/demo.git");
        assert_eq!(source.version, "1.0");
    }

    /// Direct Git acquisition requires a safe URL and a full object identity.
    #[test_case("http://example.org/demo.git", "deadbeefdeadbeefdeadbeefdeadbeefdeadbeef"; "insecure")]
    #[test_case("https://token@example.org/demo.git", "deadbeefdeadbeefdeadbeefdeadbeefdeadbeef"; "credentialed")]
    #[test_case("https://example.org/demo.git?ref=main", "deadbeefdeadbeefdeadbeefdeadbeefdeadbeef"; "query")]
    #[test_case("https://example.org/demo.git", "deadbeef"; "abbreviated_commit")]
    fn invariant_direct_vcs_requires_immutable_safe_inputs(url: &str, commit: &str) {
        let input = format!(
            "{HEADER}[[packages]]\nname = 'demo'\nversion = '1'\n[packages.vcs]\ntype = 'git'\nurl = {url:?}\ncommit-id = {commit:?}\n"
        );

        assert_eq!(
            PylockToml::parse(&input)
                .unwrap()
                .installation_fragments()
                .unwrap()[0]
                .vcs_source,
            None
        );
    }

    /// Local directories retain one portable project root for first-party mapping.
    #[test]
    fn invariant_directory_sources_preserve_their_workspace_path() {
        let input = format!(
            "{HEADER}[[packages]]\nname = 'demo'\n[packages.directory]\npath = 'packages/source'\neditable = true\nsubdirectory = 'python'\n"
        );

        let fragment = &PylockToml::parse(&input)
            .unwrap()
            .installation_fragments()
            .unwrap()[0];

        assert_eq!(
            fragment.directory_source,
            Some(PylockDirectorySource {
                editable: true,
                path: "packages/source/python".to_owned(),
            })
        );
    }

    /// Local paths cannot escape the repository or depend on host path syntax.
    #[test_case("../source"; "parent")]
    #[test_case("/source"; "absolute")]
    #[test_case("packages//source"; "empty_component")]
    #[test_case("packages\\source"; "backslash")]
    fn invariant_directory_source_path_is_normalized(path: &str) {
        let input =
            format!("{HEADER}[[packages]]\nname = 'demo'\n[packages.directory]\npath = {path:?}\n");

        assert_package_error(
            &input,
            "demo",
            "directory path must be a normalized relative path",
        );
    }

    /// Platform-varying mutable trees cannot silently enter a frozen build action.
    #[test]
    fn invariant_directory_sources_require_one_unconditional_variant() {
        let input = format!(
            "{HEADER}[[packages]]\nname = 'demo'\nmarker = \"sys_platform == 'darwin'\"\n[packages.directory]\npath = 'packages/darwin'\n[[packages]]\nname = 'demo'\nmarker = \"sys_platform == 'linux'\"\n[packages.directory]\npath = 'packages/linux'\n"
        );

        assert!(matches!(
            PylockToml::parse(&input).unwrap().installation_fragments(),
            Err(PylockTomlError::Package { package, reason })
                if package == "demo"
                    && reason == "directory sources require one unconditional variant"
        ));
    }

    /// Required source identity must fail before acquisition or package execution.
    #[test_case("[packages.vcs]\ntype = \"git\""; "missing_commit")]
    #[test_case("[packages.directory]\neditable = true"; "missing_directory_path")]
    fn invariant_source_required_fields_are_rejected(source: &str) {
        let input = format!("{HEADER}[[packages]]\nname = \"demo\"\n{source}\n");

        assert!(matches!(
            PylockToml::parse(&input),
            Err(PylockTomlError::Deserialize(_))
        ));
    }

    /// Package names must already carry their canonical distribution identity.
    #[test]
    fn invariant_package_name_is_normalized() {
        let input = format!(
            "{HEADER}[[packages]]\nname = \"Typing_Extensions\"\n[[packages.wheels]]\nurl = \"https://example.org/demo.whl\"\nhashes = {{ sha256 = \"demo\" }}\n"
        );

        assert_package_error(&input, "Typing_Extensions", "name must be normalized");
    }

    /// Package selection markers must use Astral's exact PEP 508 grammar.
    #[test]
    fn invariant_package_marker_is_valid() {
        let input = format!(
            "{HEADER}[[packages]]\nname = 'demo'\nmarker = 'sys_platform === linux'\n[[packages.wheels]]\nurl = 'https://example.org/demo.whl'\nhashes = {{ sha256 = 'demo' }}\n"
        );

        assert_package_error(&input, "demo", "marker must be a valid PEP 508 expression");
    }

    /// One package entry cannot select incompatible acquisition mechanisms.
    #[test_case("[packages.vcs]\ntype = \"git\"\nurl = \"https://example.org/demo.git\"\ncommit-id = \"deadbeef\"\n[[packages.wheels]]\nurl = \"https://example.org/demo.whl\"\nhashes = { sha256 = \"demo\" }"; "vcs_and_wheel")]
    #[test_case("[packages.archive]\nurl = \"https://example.org/demo.zip\"\nhashes = { sha256 = \"archive\" }\n[packages.sdist]\nurl = \"https://example.org/demo.tar.gz\"\nhashes = { sha256 = \"sdist\" }"; "archive_and_sdist")]
    #[test_case("[packages.directory]\npath = \"demo\"\n[packages.archive]\npath = \"demo.zip\"\nhashes = { sha256 = \"archive\" }"; "directory_and_archive")]
    fn invariant_package_source_is_unambiguous(source: &str) {
        let input = format!("{HEADER}[[packages]]\nname = \"demo\"\n{source}\n");

        assert_package_error(&input, "demo", "acquisition sources are mutually exclusive");
    }

    /// Every installable package needs one replayable acquisition source.
    #[test]
    fn invariant_package_source_is_required() {
        let input = format!("{HEADER}[[packages]]\nname = \"demo\"\nversion = \"1.0\"\n");

        assert_package_error(&input, "demo", "acquisition source is required");
    }

    /// Every selected artifact needs both a location and immutable hash identity.
    #[test_case("url = \"https://example.org/demo.whl\"\nhashes = {}", "wheel hashes must not be empty"; "empty_hashes")]
    #[test_case("hashes = { sha256 = \"demo\" }", "wheel URL or path is required"; "missing_location")]
    fn invariant_artifact_is_replayable(artifact: &str, expected: &str) {
        let input =
            format!("{HEADER}[[packages]]\nname = \"demo\"\n[[packages.wheels]]\n{artifact}\n");

        assert_package_error(&input, "demo", expected);
    }

    /// Archive project roots cannot escape or ambiguously address their source tree.
    #[test_case("../package"; "parent")]
    #[test_case("/package"; "absolute")]
    #[test_case("package//src"; "empty_component")]
    #[test_case("package\\src"; "backslash")]
    fn invariant_artifact_subdirectory_is_normalized(subdirectory: &str) {
        let input = format!(
            "{HEADER}[[packages]]\nname = 'demo'\nversion = '1'\n[packages.archive]\nurl = 'https://example.org/source.zip'\nsubdirectory = {subdirectory:?}\nhashes = {{ sha256 = 'demo' }}\n"
        );

        assert_package_error(
            &input,
            "demo",
            "artifact subdirectory must be a normalized relative path",
        );
    }

    /// VCS entries need an immutable revision and one acquisition location.
    #[test_case("url = \"https://example.org/demo.git\"\ncommit-id = \"\"", "VCS commit ID must not be empty"; "empty_commit")]
    #[test_case("commit-id = \"deadbeef\"", "VCS URL or path is required"; "missing_location")]
    fn invariant_vcs_source_is_replayable(vcs: &str, expected: &str) {
        let input = format!(
            "{HEADER}[[packages]]\nname = \"demo\"\n[packages.vcs]\ntype = \"git\"\n{vcs}\n"
        );

        assert_package_error(&input, "demo", expected);
    }

    /// Attestation entries without an identity kind cannot support provenance.
    #[test]
    fn invariant_attestation_identity_has_kind() {
        let input = format!(
            "{HEADER}[[packages]]\nname = \"demo\"\n[[packages.wheels]]\nurl = \"https://example.org/demo.whl\"\nhashes = {{ sha256 = \"demo\" }}\n[[packages.attestation-identities]]\npublisher = \"example\"\n"
        );

        assert_package_error(&input, "demo", "attestation identity kind is required");
    }

    /// Installation fragments isolate package changes while retaining marker variants.
    #[test]
    fn invariant_installation_fragments_are_package_granular() {
        let input = format!(
            "{}future-top-level = 'kept'\n{}{}{}",
            HEADER,
            TYPING,
            ATTRS,
            TYPING.replace(
                "name = \"typing-extensions\"",
                "name = \"typing-extensions\"\nmarker = \"python_version < '3.13'\"\nfuture-package-field = 'kept'"
            )
        );
        let lock = PylockToml::parse(&input).unwrap();
        let fragments = lock.installation_fragments().unwrap();

        assert_eq!(
            fragments
                .iter()
                .map(|fragment| fragment.package.as_str())
                .collect::<Vec<_>>(),
            ["attrs", "typing-extensions"]
        );
        assert_eq!(fragments[0].contents.matches("[[packages]]").count(), 1);
        assert_eq!(fragments[1].contents.matches("[[packages]]").count(), 2);
        assert!(!fragments[0].contents.contains("typing-extensions"));
        assert!(!fragments[0].contents.contains("dependencies"));
        assert!(fragments[1].contents.contains("future-top-level"));
        assert!(fragments[1].contents.contains("future-package-field"));
        assert_eq!(fragments[0].acquisition, PylockAcquisition::Wheel);
        assert_eq!(fragments[1].acquisition, PylockAcquisition::Wheel);
        PylockToml::parse(&fragments[0].contents).unwrap();
        PylockToml::parse(&fragments[1].contents).unwrap();
    }

    /// A mixed lock delegates platform-tag selection to the pinned uv action.
    #[test]
    fn invariant_wheel_with_sdist_requires_explicit_selection() {
        let input = format!(
            "{HEADER}[[packages]]\nname = 'demo'\n[packages.sdist]\nurl = 'https://example.org/demo.tar.gz'\nhashes = {{ sha256 = 'source' }}\n[[packages.wheels]]\nurl = 'https://example.org/demo.whl'\nhashes = {{ sha256 = 'wheel' }}\n"
        );

        assert_eq!(
            PylockToml::parse(&input)
                .unwrap()
                .installation_fragments()
                .unwrap()[0]
                .acquisition,
            PylockAcquisition::WheelOrSource
        );
    }

    /// A Python 3 universal wheel cannot require platform-sensitive selection.
    #[test]
    fn invariant_universal_wheel_bypasses_explicit_selection() {
        let wheels = format!(
            "[packages.sdist]\nurl = 'https://example.org/demo.tar.gz'\nhashes = {{ sha256 = 'source' }}\n{}",
            locked_wheel("https://example.org/demo-1-py3-none-any.whl")
        );
        let fragment = direct_fragment("", &wheels);

        assert_eq!(fragment.acquisition, PylockAcquisition::Wheel);
        assert_eq!(
            fragment.artifact,
            Some(PylockArtifact {
                filename: "demo-1-py3-none-any.whl".to_owned(),
                sha256: SHA256.to_owned(),
                size: 42,
                url: "https://example.org/demo-1-py3-none-any.whl".to_owned(),
            })
        );
    }

    /// A complete remote sdist becomes a verified input before PEP 517 executes.
    #[test]
    fn invariant_remote_sdist_is_declared_acquisition_input() {
        let input = format!(
            "{HEADER}[[packages]]\nname = 'demo'\nversion = '1'\nmarker = \"sys_platform == 'darwin' or sys_platform == 'linux'\"\n[packages.sdist]\nurl = 'https://example.org/demo-1.tar.gz'\nsize = 42\nhashes = {{ sha256 = {SHA256:?} }}\n"
        );
        let fragment = &PylockToml::parse(&input)
            .unwrap()
            .installation_fragments()
            .unwrap()[0];

        assert_eq!(fragment.acquisition, PylockAcquisition::Source);
        assert_eq!(
            fragment.source_artifact,
            Some(PylockSourceArtifact {
                artifact: PylockArtifact {
                    filename: "demo-1.tar.gz".to_owned(),
                    sha256: SHA256.to_owned(),
                    size: 42,
                    url: "https://example.org/demo-1.tar.gz".to_owned(),
                },
                subdirectory: None,
                version: "1".to_owned(),
            })
        );
    }

    /// Direct sdist acquisition requires the locked identity and archive root.
    #[test_case("other-1.tar.gz", ""; "distribution")]
    #[test_case("demo-2.tar.gz", ""; "version")]
    #[test_case("demo-1.tar.gz", "subdirectory = 'package'\n"; "subdirectory")]
    fn invariant_direct_sdist_matches_lock_identity(filename: &str, fields: &str) {
        let input = format!(
            "{HEADER}[[packages]]\nname = 'demo'\nversion = '1'\n[packages.sdist]\nurl = 'https://example.org/{filename}'\nsize = 42\nhashes = {{ sha256 = {SHA256:?} }}\n{fields}"
        );

        assert_eq!(
            PylockToml::parse(&input)
                .unwrap()
                .installation_fragments()
                .unwrap()[0]
                .source_artifact,
            None
        );
    }

    /// Direct acquisition requires one unambiguous URL and SHA-256 identity.
    #[test_case("file:///tmp/demo-1-py3-none-any.whl"; "local")]
    #[test_case("https://token@example.org/demo-1-py3-none-any.whl"; "credentialed")]
    #[test_case("https://example.org/demo-1-py3-none-any.whl?token=secret"; "query")]
    #[test_case("https://example.org/demo%2D1-py3-none-any.whl"; "encoded_filename")]
    fn invariant_direct_wheel_requires_safe_url(url: &str) {
        assert_eq!(direct_fragment("", &locked_wheel(url)).artifact, None);
    }

    /// Missing size or strong digest leaves acquisition with uv.
    #[test]
    fn invariant_direct_wheel_requires_complete_metadata() {
        let url = "https://example.org/demo-1-py3-none-any.whl";
        for wheel in [
            format!("[[packages.wheels]]\nurl = {url:?}\nhashes = {{ sha256 = {SHA256:?} }}\n"),
            format!(
                "[[packages.wheels]]\nurl = {url:?}\nsize = 42\nhashes = {{ sha256 = 'invalid' }}\n"
            ),
        ] {
            assert_eq!(direct_fragment("", &wheel).artifact, None);
        }
    }

    /// A direct wheel must identify the locked distribution and version.
    #[test_case("other-1-py3-none-any.whl"; "distribution")]
    #[test_case("demo-2-py3-none-any.whl"; "version")]
    #[test_case("demo-1-1-py3-none-any.whl"; "build_tag")]
    fn invariant_direct_wheel_matches_lock_identity(filename: &str) {
        assert_eq!(
            direct_fragment(
                "",
                &locked_wheel(&format!("https://example.org/{filename}"))
            )
            .artifact,
            None
        );
    }

    /// Marker-qualified lock variants stay delegated to uv selection.
    #[test]
    fn invariant_direct_wheel_requires_one_lock_variant() {
        let wheel = locked_wheel("https://example.org/demo-1-py3-none-any.whl");
        let input = format!(
            "{HEADER}[[packages]]\nname = 'demo'\nversion = '1'\nmarker = \"python_version >= '3.14'\"\n{wheel}[[packages]]\nname = 'demo'\nversion = '1'\nmarker = \"python_version < '3.14'\"\n{wheel}"
        );

        assert_eq!(
            PylockToml::parse(&input)
                .unwrap()
                .installation_fragments()
                .unwrap()[0]
                .artifact,
            None
        );
    }

    /// Platform alternatives stay delegated to uv's wheel-tag ordering.
    #[test]
    fn invariant_direct_wheel_requires_one_artifact_candidate() {
        let mut wheels = locked_wheel("https://example.org/demo-1-py3-none-any.whl");
        wheels.push_str(&locked_wheel(
            "https://example.org/demo-1-cp314-cp314-macosx_15_0_arm64.whl",
        ));

        assert_eq!(direct_fragment("", &wheels).artifact, None);
    }

    /// Every target receives only wheels compatible with one supported CPython line.
    #[test]
    fn invariant_platform_wheels_are_partitioned_before_acquisition() {
        let wheels = [
            "demo-1-cp313-cp313-macosx_13_0_arm64.whl",
            "demo-1-cp314-cp314-macosx_13_0_arm64.whl",
            "demo-1-cp313-cp313-manylinux_2_28_x86_64.whl",
            "demo-1-cp314-cp314-manylinux_2_28_x86_64.whl",
            "demo-1-py3-none-any.whl",
        ]
        .into_iter()
        .map(|filename| locked_wheel(&format!("https://example.org/{filename}")))
        .collect::<String>();

        let fragment = direct_fragment("", &wheels);

        for version in ["3.13", "3.14"] {
            assert_eq!(
                fragment.artifacts[&format!("{version}-macos-arm64")].len(),
                2
            );
            assert_eq!(
                fragment.artifacts[&format!("{version}-linux-x86_64")].len(),
                2
            );
            assert_eq!(
                fragment.artifacts[&format!("{version}-macos-x86_64")].len(),
                1
            );
            assert_eq!(
                fragment.artifacts[&format!("{version}-linux-arm64")].len(),
                1
            );
        }
    }

    /// A marker that covers BSMR's complete Python platform domain is not a selection.
    #[test_case("sys_platform == 'darwin' or sys_platform == 'linux'"; "operating_system")]
    #[test_case("(platform_python_implementation != 'PyPy' and sys_platform == 'darwin') or (platform_python_implementation != 'PyPy' and sys_platform == 'linux')"; "implementation")]
    fn invariant_direct_wheel_accepts_supported_platform_domain(marker: &str) {
        let fragment = direct_fragment(
            &format!("marker = {marker:?}\n"),
            &locked_wheel("https://example.org/demo-1-py3-none-any.whl"),
        );

        assert!(fragment.artifact.is_some());
    }

    /// Direct acquisition cannot bypass package-level environment selection.
    #[test_case("marker = \"sys_platform == 'darwin'\""; "platform_marker")]
    #[test_case("marker = \"python_version >= '3.14'\""; "version_marker")]
    #[test_case("requires-python = '>=3.14'"; "requires_python")]
    fn invariant_direct_wheel_rejects_package_selection(selection: &str) {
        assert_eq!(
            direct_fragment(
                &format!("{selection}\n"),
                &locked_wheel("https://example.org/demo-1-py3-none-any.whl")
            )
            .artifact,
            None
        );
    }

    /// Unrelated lock edits must not invalidate another package action identity.
    #[test]
    fn invariant_installation_fragment_ignores_unrelated_packages() {
        let before = PylockToml::parse(&format!("{HEADER}{ATTRS}{TYPING}"))
            .unwrap()
            .installation_fragments()
            .unwrap();
        let after = PylockToml::parse(&format!(
            "{HEADER}{ATTRS}{}",
            TYPING.replace("typing.whl", "typing-repacked.whl")
        ))
        .unwrap()
        .installation_fragments()
        .unwrap();

        assert_eq!(before[0].package, "attrs");
        assert_eq!(before[0].contents, after[0].contents);
        assert_ne!(before[1].contents, after[1].contents);
    }
}
