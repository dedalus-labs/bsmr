//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Publishes locked Git objects as verified source archives without running checkout hooks.

use std::io::Seek;
use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use cargo_util::Sha256;
use flate2::Compression;
use flate2::write::GzEncoder;
use git2::ObjectType;
use git2::Repository;
use git2::Tree;

use crate::types::SourceArtifact;

/// Export object bytes, never mutable files from Cargo's checked-out source cache.
pub(crate) fn archive(
    repository: &Repository,
    tree: &Tree<'_>,
    home: &Path,
    package: &Path,
) -> Result<SourceArtifact> {
    let directory = home.join("source-archives");
    std::fs::create_dir_all(&directory)?;
    let mut temporary = tempfile::NamedTempFile::new_in(&directory)?;
    let encoder = GzEncoder::new(temporary.as_file_mut(), Compression::fast());
    let mut archive = tar::Builder::new(encoder);
    append(&mut archive, repository, tree, Path::new("source"))?;
    archive.into_inner()?.finish()?;
    temporary.as_file_mut().rewind()?;
    let mut hash = Sha256::new();
    hash.update_file(temporary.as_file_mut())?;
    let sha256 = hash.finish_hex();
    let size = temporary.as_file().metadata()?.len();
    let path = directory.join(format!("{sha256}.tar.gz"));
    temporary.persist(&path)?;
    let url =
        url::Url::from_file_path(path).map_err(|()| anyhow::anyhow!("invalid archive path"))?;
    Ok(SourceArtifact::Archive {
        url: url.into(),
        sha256,
        size,
        prefix: Path::new("source")
            .join(package)
            .to_string_lossy()
            .into_owned(),
    })
}

/// Preserve executable bits and symlinks while refusing unowned submodule contents.
fn append<W: std::io::Write>(
    archive: &mut tar::Builder<W>,
    repository: &Repository,
    tree: &Tree<'_>,
    prefix: &Path,
) -> Result<()> {
    for entry in tree {
        let path = prefix.join(entry.name().context("non-UTF-8 Git source path")?);
        match entry.kind() {
            Some(ObjectType::Tree) => {
                append(
                    archive,
                    repository,
                    &repository.find_tree(entry.id())?,
                    &path,
                )?;
            }
            Some(ObjectType::Blob) => {
                let blob = repository.find_blob(entry.id())?;
                let mut header = tar::Header::new_gnu();
                header.set_mode(u32::try_from(entry.filemode())? & 0o777);
                header.set_mtime(0);
                if entry.filemode() == 0o120000 {
                    header.set_entry_type(tar::EntryType::Symlink);
                    header.set_size(0);
                    let target =
                        std::str::from_utf8(blob.content()).context("non-UTF-8 Git symlink")?;
                    archive.append_link(&mut header, &path, target)?;
                } else {
                    ensure!(
                        entry.filemode() & 0o170000 == 0o100000,
                        "invalid Git file mode"
                    );
                    header.set_size(u64::try_from(blob.size())?);
                    header.set_cksum();
                    archive.append_data(&mut header, &path, blob.content())?;
                }
            }
            _ => anyhow::bail!(
                "Git source contains an unsupported submodule: {}",
                path.display()
            ),
        }
    }
    Ok(())
}

#[cfg(all(test, unix))]
mod tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::os::unix::fs::symlink;

    use flate2::read::GzDecoder;

    use super::*;

    /// Mutable checkout bytes and export attributes cannot change a locked source tree.
    #[test]
    fn invariant_archive_preserves_locked_objects() -> Result<()> {
        let root = tempfile::tempdir()?;
        let repository = Repository::init(root.path())?;
        fs::write(root.path().join("lib.rs"), "pub const VALUE: u8 = 17;\n")?;
        fs::write(root.path().join(".gitattributes"), "lib.rs export-ignore\n")?;
        fs::set_permissions(
            root.path().join("lib.rs"),
            fs::Permissions::from_mode(0o755),
        )?;
        symlink("lib.rs", root.path().join("alias"))?;
        let mut index = repository.index()?;
        index.add_all(["*"], git2::IndexAddOption::DEFAULT, None)?;
        let tree = repository.find_tree(index.write_tree()?)?;
        fs::write(root.path().join("lib.rs"), "wrong checkout contents\n")?;
        let home = tempfile::tempdir()?;
        let SourceArtifact::Archive {
            url,
            sha256,
            size,
            prefix,
        } = archive(&repository, &tree, home.path(), Path::new(""))?
        else {
            anyhow::bail!("expected source archive");
        };
        let path = url::Url::parse(&url)?
            .to_file_path()
            .expect("local archive");
        let file = fs::File::open(&path)?;
        assert_eq!(Sha256::new().update_file(&file)?.finish_hex(), sha256);
        assert_eq!(file.metadata()?.len(), size);
        let restored = tempfile::tempdir()?;
        tar::Archive::new(GzDecoder::new(fs::File::open(path)?)).unpack(restored.path())?;
        let source = restored.path().join(prefix);
        assert_eq!(
            fs::read_to_string(source.join("lib.rs"))?,
            "pub const VALUE: u8 = 17;\n"
        );
        assert_eq!(fs::read_link(source.join("alias"))?, Path::new("lib.rs"));
        assert_ne!(
            fs::metadata(source.join("lib.rs"))?.permissions().mode() & 0o111,
            0
        );
        let SourceArtifact::Archive {
            sha256: repeated, ..
        } = archive(&repository, &tree, home.path(), Path::new(""))?
        else {
            anyhow::bail!("expected source archive");
        };
        assert_eq!(sha256, repeated);
        Ok(())
    }
}
