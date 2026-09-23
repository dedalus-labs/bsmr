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

use bsmr_core::fs::project::ProjectRoot;
use bsmr_core::fs::project_rel_path::ProjectRelativePathBuf;
use bsmr_directory::directory::directory::Directory;
use bsmr_directory::directory::entry::DirectoryEntry;
use bsmr_execute::directory::ActionDirectory;
use bsmr_execute::directory::ActionDirectoryEntry;
use bsmr_execute::directory::ActionDirectoryMember;
use bsmr_execute::directory::ActionDirectoryRef;
use bsmr_execute::directory::ActionSharedDirectory;
use bsmr_execute::execute::blocking::IoRequest;
use bsmr_fs::error::IoResultExt;
use bsmr_fs::fs_util;
use bsmr_fs::paths::RelativePath;
use bsmr_fs::paths::abs_norm_path::AbsNormPath;
use bsmr_fs::paths::abs_norm_path::AbsNormPathBuf;
use bsmr_hash::StdBsmrHashMap;

pub struct MaterializeTreeStructure {
    pub path: ProjectRelativePathBuf,
    pub entry: ActionDirectoryEntry<ActionSharedDirectory>,
}

#[derive(Clone, Copy)]
struct MaterializeOptions {
    directories_and_symlinks: bool,
    executable_bit_override: Option<bool>,
    external_output_root: bool,
}

impl IoRequest for MaterializeTreeStructure {
    fn execute(self: Box<Self>, project_fs: &ProjectRoot) -> bsmr_error::Result<()> {
        let output_root = project_fs
            .root()
            .join(bsmr_core::fs::project_rel_path::ProjectRelativePath::unchecked_new("bsmr-out"));
        let external_output_root = fs_util::symlink_metadata_if_exists(&output_root)?
            .is_some_and(|metadata| metadata.file_type().is_symlink());
        materialize_dirs_and_syms(
            self.entry.as_ref(),
            project_fs.root().join(&self.path),
            external_output_root,
        )?;

        Ok(())
    }
}

/// Materializes the entry at `dest`.
///
/// - `file_src`: takes the destination path of a file, and returns its
///   source path (where it should be copied from). If it returns [`None`],
///   the file is not materialized.
fn materialize<F, D>(
    entry: DirectoryEntry<&D, &ActionDirectoryMember>,
    dest: &AbsNormPath,
    mut file_src: F,
    options: MaterializeOptions,
) -> bsmr_error::Result<()>
where
    F: FnMut(&AbsNormPath) -> Option<AbsNormPathBuf>,
    D: ActionDirectory,
{
    let mut dest = dest.to_owned();
    if options.directories_and_symlinks {
        // create the directory where we'll materialize the entry
        if let Some(parent) = dest.parent() {
            fs_util::create_dir_all(parent)?;
        }
    }
    materialize_recursively(
        entry.map_dir(|d| Directory::as_ref(d)),
        &mut dest,
        &mut file_src,
        options,
    )
}

/// Materializes the directories and symlinks of an entry at `dest`. Files
/// are not materialized.
pub(crate) fn materialize_dirs_and_syms<P, D>(
    entry: DirectoryEntry<&D, &ActionDirectoryMember>,
    dest: P,
    external_output_root: bool,
) -> bsmr_error::Result<()>
where
    P: AsRef<AbsNormPath>,
    D: ActionDirectory,
{
    materialize(
        entry,
        dest.as_ref(),
        |_: &AbsNormPath| None,
        MaterializeOptions {
            directories_and_symlinks: true,
            executable_bit_override: None,
            external_output_root,
        },
    )
}

/// Materializes the files of an the entry rooted at `dest`.
///
/// Files are copied from `src`. In other words, if a file would be
/// materialized at `dest/p`, then it's copied from `src/p`.
pub(crate) fn materialize_files<P, D>(
    entry: DirectoryEntry<&D, &ActionDirectoryMember>,
    src: P,
    dest: P,
    executable_bit_override: Option<bool>,
) -> bsmr_error::Result<()>
where
    P: AsRef<AbsNormPath>,
    D: ActionDirectory,
{
    let src = src.as_ref();
    let dest = dest.as_ref();
    let file_src = |d: &AbsNormPath| {
        // It's safe to unwrap because `materialize_impl` always gives us a
        // path inside `dest`.
        let subpath = d.strip_prefix(dest).unwrap();
        if subpath.as_str().is_empty() {
            // `dest` itself is a file
            Some(src.to_buf())
        } else {
            Some(src.join(subpath))
        }
    };
    materialize(
        entry,
        dest,
        file_src,
        MaterializeOptions {
            directories_and_symlinks: false,
            executable_bit_override,
            external_output_root: false,
        },
    )
}

/// Materializes the files of an entry rooted at `dest`.
///
/// For a file at path `file_dest` in the entry, if `file_dest` exists in
/// `srcs` with value `file_src`, the file is copied from `file_src` to
/// `file_dest`. It's then removed from `srcs`.
fn _materialize_files_from_map<P, D>(
    entry: DirectoryEntry<&D, &ActionDirectoryMember>,
    srcs: &mut StdBsmrHashMap<AbsNormPathBuf, AbsNormPathBuf>,
    dest: P,
) -> bsmr_error::Result<()>
where
    P: AsRef<AbsNormPath>,
    D: ActionDirectory,
{
    let file_src = |d: &AbsNormPath| srcs.remove(d);
    materialize(
        entry,
        dest.as_ref(),
        file_src,
        MaterializeOptions {
            directories_and_symlinks: false,
            executable_bit_override: None,
            external_output_root: false,
        },
    )
}

fn materialize_recursively<'a, F, D>(
    entry: DirectoryEntry<D, &ActionDirectoryMember>,
    dest: &mut AbsNormPathBuf,
    file_src: &mut F,
    options: MaterializeOptions,
) -> bsmr_error::Result<()>
where
    F: FnMut(&AbsNormPath) -> Option<AbsNormPathBuf>,
    D: ActionDirectoryRef<'a>,
{
    match entry {
        DirectoryEntry::Dir(d) => {
            if options.directories_and_symlinks {
                fs_util::create_dir_all(&dest)?;
            }
            for (name, entry) in d.entries() {
                dest.push(name);
                materialize_recursively(entry, dest, file_src, options)?;
                dest.pop();
            }
            Ok(())
        }
        DirectoryEntry::Leaf(ActionDirectoryMember::File(_)) => {
            if let Some(src) = file_src(dest) {
                fs_util::copy(src, &dest).categorize_internal()?;
                if let Some(executable_bit_override) = options.executable_bit_override {
                    fs_util::set_executable(&dest, executable_bit_override)
                        .categorize_internal()?;
                }
            }
            Ok(())
        }
        DirectoryEntry::Leaf(ActionDirectoryMember::Symlink(s)) => {
            if options.directories_and_symlinks
                && fs_util::symlink_metadata(&dest)
                    .categorize_internal()
                    .is_err()
            {
                let target =
                    materialized_symlink_target(dest, s.target(), options.external_output_root)?;
                fs_util::symlink(target, dest).categorize_internal()?;
            }
            Ok(())
        }
        DirectoryEntry::Leaf(ActionDirectoryMember::ExternalSymlink(s)) => {
            if options.directories_and_symlinks
                && fs_util::symlink_metadata(&dest)
                    .categorize_internal()
                    .is_err()
            {
                fs_util::symlink(s.target(), dest).categorize_internal()?;
            }
            Ok(())
        }
    }
}

/// Keeps portable logical links in the graph while fixing their physical managed-view target.
fn materialized_symlink_target(
    destination: &AbsNormPath,
    target: &RelativePath,
    external_output_root: bool,
) -> bsmr_error::Result<std::path::PathBuf> {
    if !external_output_root {
        return Ok(target.as_str().into());
    }
    destination
        .parent()
        .expect("materialized symlinks always have a parent")
        .join_normalized(target)
        .map(AbsNormPathBuf::into_path_buf)
}

#[cfg(test)]
mod tests {
    use bsmr_fs::paths::abs_norm_path::AbsNormPath;
    use bsmr_fs::paths::abs_norm_path::AbsNormPathBuf;
    use bsmr_fs::paths::relative_path::RelativePath;

    use super::materialized_symlink_target;

    #[test]
    fn invariant_managed_output_links_still_resolve_to_project_sources() -> bsmr_error::Result<()> {
        let (destination, expected) = if cfg!(windows) {
            (
                "C:/project/bsmr-out/default/art/root/hash/dependencies/src/package.json",
                "C:/project/package.json",
            )
        } else {
            (
                "/project/bsmr-out/default/art/root/hash/dependencies/src/package.json",
                "/project/package.json",
            )
        };
        let destination = AbsNormPath::new(destination)?;
        let target = RelativePath::unchecked_new("../../../../../../../package.json");

        let materialized = materialized_symlink_target(destination, target, true)?;
        let ordinary = materialized_symlink_target(destination, target, false)?;

        assert_eq!(materialized, std::path::Path::new(expected));
        assert_eq!(
            ordinary,
            std::path::Path::new("../../../../../../../package.json")
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn invariant_materialized_link_resolves_through_external_view() -> bsmr_error::Result<()> {
        let temporary = tempfile::tempdir()?;
        let project = temporary.path().join("project");
        let view = temporary.path().join("view");
        std::fs::create_dir(&project)?;
        std::fs::create_dir(&view)?;
        std::os::unix::fs::symlink(&view, project.join("bsmr-out"))?;
        std::fs::write(project.join("package.json"), b"source bytes")?;
        let destination = AbsNormPathBuf::new(
            project.join("bsmr-out/default/art/root/hash/dependencies/src/package.json"),
        )?;
        std::fs::create_dir_all(
            destination
                .parent()
                .expect("materialized output always has a parent"),
        )?;
        let target = RelativePath::unchecked_new("../../../../../../../package.json");
        let target = materialized_symlink_target(&destination, target, true)?;

        std::os::unix::fs::symlink(target, &destination)?;

        assert_eq!(std::fs::read(destination)?, b"source bytes");
        Ok(())
    }
}
