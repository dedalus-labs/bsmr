//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Binds resolved packages to verified registry archives and pinned Git manifests.

use std::fs::File;
use std::io::Read;
use std::io::Seek;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use cargo::GlobalContext;
use cargo::core::PackageId;
use cargo::core::Resolve;
use cargo::core::SourceId;
use cargo::core::SourceKind as CargoSourceKind;
use cargo::core::compiler::Unit;
use cargo::sources::SourceConfigMap;
use cargo::sources::registry::RegistrySource;
use cargo::util::cache_lock::CacheLockMode;
use cargo_util::Sha256;
use flate2::read::GzDecoder;

use crate::types::Archive;
use crate::types::Source;
use crate::types::SourceArtifact;
use crate::types::SourceKind;

#[derive(Debug, thiserror::Error)]
enum ArchiveError {
    #[error("registry package {0} has no locked checksum")]
    MissingChecksum(String),
    #[error("acquired registry archive unavailable at {path}: {source}")]
    Unavailable {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("registry source replacement has no supported archive: {0}")]
    UnsupportedSource(String),
    #[error("archive checksum mismatch for {path}: expected {expected}, got {actual}")]
    Checksum {
        path: PathBuf,
        expected: String,
        actual: String,
    },
    #[error("cached manifest differs from the locked archive for {0}")]
    Manifest(String),
}

#[derive(Debug, thiserror::Error)]
#[error("cached Git manifest differs from the locked commit: {0}")]
struct GitManifestError(PathBuf);

/// Preserve Cargo source identity and verify manifests against their locked source.
pub(crate) fn export(unit: &Unit, resolve: &Resolve, gctx: &GlobalContext) -> Result<Source> {
    let source_id = unit.pkg.package_id().source_id();
    if matches!(source_id.kind(), CargoSourceKind::Directory) {
        return Err(ArchiveError::UnsupportedSource(source_id.as_url().to_string()).into());
    }
    let checksum = resolve
        .checksums()
        .get(&unit.pkg.package_id())
        .cloned()
        .flatten();
    let (archive, artifact) = if source_id.is_registry() {
        let checksum = checksum
            .as_deref()
            .ok_or_else(|| ArchiveError::MissingChecksum(unit.pkg.to_string()))?;
        let (archive, artifact) = archive(unit, checksum, gctx, resolve)?;
        (Some(archive), artifact)
    } else if source_id.is_git() {
        (None, git_manifests(unit, gctx)?)
    } else {
        (None, SourceArtifact::Workspace)
    };
    Ok(Source {
        artifact,
        kind: match source_id.kind() {
            CargoSourceKind::Path => SourceKind::Path,
            CargoSourceKind::Git(_) => SourceKind::Git,
            CargoSourceKind::Registry => SourceKind::Registry,
            CargoSourceKind::SparseRegistry => SourceKind::SparseRegistry,
            CargoSourceKind::LocalRegistry => SourceKind::LocalRegistry,
            CargoSourceKind::Directory => SourceKind::Directory,
        },
        identity: source_id.as_url().to_string(),
        checksum,
        archive,
        git_revision: source_id.precise_git_fragment().map(str::to_owned),
        root: unit.pkg.root().to_owned(),
        manifest: unit.pkg.manifest_path().to_owned(),
    })
}

/// Use the pinned Cargo registry cache key, never discover identity from directories.
fn archive(
    unit: &Unit,
    checksum: &str,
    gctx: &GlobalContext,
    resolve: &Resolve,
) -> Result<(Archive, SourceArtifact)> {
    let package = unit.pkg.package_id();
    let source =
        SourceConfigMap::new(gctx)?.load(package.source_id(), &resolve.iter().collect())?;
    let acquired = source.replaced_source_id();
    if !acquired.is_remote_registry() {
        return Err(ArchiveError::UnsupportedSource(acquired.as_url().to_string()).into());
    }
    // Cargo 0.98 sources/registry/mod.rs::short_name uses this exact key.
    let host = acquired
        .url()
        .host_str()
        .ok_or_else(|| ArchiveError::UnsupportedSource(acquired.as_url().to_string()))?;
    let key = format!("{host}-{}", cargo::util::hex::short_hash(&acquired));
    let path = gctx
        .registry_cache_path()
        .join(key)
        .join(package.tarball_name())
        .into_path_unlocked();
    let mut file = File::open(&path).map_err(|source| ArchiveError::Unavailable {
        path: path.clone(),
        source,
    })?;
    let size = file.metadata()?.len();
    let actual = Sha256::new().update_file(&file)?.finish_hex();
    if actual != checksum {
        return Err(ArchiveError::Checksum {
            path,
            expected: checksum.to_owned(),
            actual,
        }
        .into());
    }
    file.rewind()?;
    let manifest = PathBuf::from(format!(
        "{}-{}/Cargo.toml",
        package.name(),
        package.version()
    ));
    for entry in tar::Archive::new(GzDecoder::new(file)).entries()? {
        let mut entry = entry?;
        if entry.path()? != manifest {
            continue;
        }
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes)?;
        if !entry.header().entry_type().is_file()
            || bytes != std::fs::read(unit.pkg.manifest_path())?
        {
            return Err(ArchiveError::Manifest(package.to_string()).into());
        }
        let artifact = registry_artifact(package, acquired, checksum, size, gctx)?;
        return Ok((Archive { path, size }, artifact));
    }
    Err(ArchiveError::Manifest(package.to_string()).into())
}

/// Resolve the registry's download endpoint while holding Cargo's cache ownership lock.
fn registry_artifact(
    package: PackageId,
    source: SourceId,
    checksum: &str,
    size: u64,
    gctx: &GlobalContext,
) -> Result<SourceArtifact> {
    let registry = RegistrySource::remote(source, &Default::default(), gctx)?;
    let _lock = gctx.acquire_package_cache_lock(CacheLockMode::DownloadExclusive)?;
    let config = futures::executor::block_on(registry.config())?
        .context("remote registry has no download configuration")?;
    ensure!(
        !config.auth_required,
        "native registry downloads require credential-free URLs"
    );
    Ok(SourceArtifact::Archive {
        url: cargo_util::registry::crate_url(
            &config.dl,
            &package.name(),
            &package.version().to_string(),
            checksum,
        ),
        sha256: checksum.to_owned(),
        size,
        prefix: format!("{}-{}", package.name(), package.version()),
        package: String::new(),
    })
}

/// Verify package and ancestor manifests because workspace inheritance reads both.
fn git_manifests(unit: &Unit, gctx: &GlobalContext) -> Result<SourceArtifact> {
    let checkouts = gctx
        .git_checkouts_path()
        .as_path_unlocked()
        .canonicalize()?;
    let package = unit.pkg.root().canonicalize()?;
    ensure!(
        package.starts_with(&checkouts),
        "Git package is outside the owned checkouts"
    );
    let git = git2::Repository::discover_path(&package, [&checkouts])?;
    let repository = git2::Repository::open(git)?;
    let root = repository
        .workdir()
        .context("Git source has no worktree")?
        .canonicalize()?;
    ensure!(
        root.starts_with(&checkouts),
        "Git worktree is outside the owned checkouts"
    );
    let revision = unit
        .pkg
        .package_id()
        .source_id()
        .precise_git_fragment()
        .context("Git source has no locked commit")?;
    let tree = repository
        .find_commit(git2::Oid::from_str(revision)?)?
        .tree()?;
    for directory in package
        .ancestors()
        .take_while(|path| path.starts_with(&root))
    {
        let manifest = directory.join("Cargo.toml");
        let cached = match std::fs::symlink_metadata(&manifest) {
            Ok(metadata) => Some(metadata),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(error) => return Err(error.into()),
        };
        let entry = match tree.get_path(manifest.strip_prefix(&root)?) {
            Ok(entry) => Some(entry),
            Err(error) if error.code() == git2::ErrorCode::NotFound => None,
            Err(error) => return Err(error.into()),
        };
        match (entry, cached) {
            (None, None) => (),
            (Some(entry), Some(metadata))
                if metadata.is_file() && entry.filemode() & 0o170000 == 0o100000 =>
            {
                let blob = repository.find_blob(entry.id())?;
                if blob.content() != std::fs::read(&manifest)? {
                    return Err(GitManifestError(manifest).into());
                }
            }
            _ => return Err(GitManifestError(manifest).into()),
        }
    }
    crate::git::archive(
        &repository,
        &tree,
        gctx.home().as_path_unlocked(),
        package.strip_prefix(&root)?,
    )
}
