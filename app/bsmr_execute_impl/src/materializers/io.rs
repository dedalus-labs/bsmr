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
use bsmr_fs::paths::abs_norm_path::AbsNormPath;
use bsmr_fs::paths::abs_norm_path::AbsNormPathBuf;
use bsmr_hash::StdBsmrHashMap;

pub struct MaterializeTreeStructure {
    pub path: ProjectRelativePathBuf,
    pub entry: ActionDirectoryEntry<ActionSharedDirectory>,
}

impl IoRequest for MaterializeTreeStructure {
    fn execute(self: Box<Self>, project_fs: &ProjectRoot) -> bsmr_error::Result<()> {
        materialize_dirs_and_syms(self.entry.as_ref(), project_fs, &self.path)?;

        Ok(())
    }
}

/// Materializes the entry at `dest`.
///
/// - `materialize_dirs_and_syms`: if `true`, materializes directories and
///   symlinks.
/// - `file_src`: takes the destination path of a file, and returns its
///   source path (where it should be copied from). If it returns [`None`],
///   the file is not materialized.
fn materialize<F, D>(
    entry: DirectoryEntry<&D, &ActionDirectoryMember>,
    dest: &AbsNormPath,
    logical_context: Option<(&ProjectRoot, ProjectRelativePathBuf)>,
    materialize_dirs_and_syms: bool,
    mut file_src: F,
    executable_bit_override: Option<bool>,
) -> bsmr_error::Result<()>
where
    F: FnMut(&AbsNormPath) -> Option<AbsNormPathBuf>,
    D: ActionDirectory,
{
    let mut dest = dest.to_owned();
    let mut logical_context = logical_context;
    if materialize_dirs_and_syms {
        // create the directory where we'll materialize the entry
        if let Some(parent) = dest.parent() {
            fs_util::create_dir_all(parent)?;
        }
    }
    materialize_recursively(
        entry.map_dir(|d| Directory::as_ref(d)),
        &mut dest,
        &mut logical_context,
        materialize_dirs_and_syms,
        &mut file_src,
        executable_bit_override,
    )
}

/// Materializes the directories and symlinks of an entry at `logical_dest`. Files
/// are not materialized.
pub(crate) fn materialize_dirs_and_syms<D>(
    entry: DirectoryEntry<&D, &ActionDirectoryMember>,
    project_fs: &ProjectRoot,
    logical_dest: &ProjectRelativePathBuf,
) -> bsmr_error::Result<()>
where
    D: ActionDirectory,
{
    let dest = project_fs.resolve(logical_dest);
    materialize(
        entry,
        &dest,
        Some((project_fs, logical_dest.clone())),
        true,
        |_: &AbsNormPath| None,
        None,
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
    materialize(entry, dest, None, false, file_src, executable_bit_override)
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
    materialize(entry, dest.as_ref(), None, false, file_src, None)
}

fn materialize_recursively<'a, F, D>(
    entry: DirectoryEntry<D, &ActionDirectoryMember>,
    dest: &mut AbsNormPathBuf,
    logical_context: &mut Option<(&ProjectRoot, ProjectRelativePathBuf)>,
    materialize_dirs_and_syms: bool,
    file_src: &mut F,
    executable_bit_override: Option<bool>,
) -> bsmr_error::Result<()>
where
    F: FnMut(&AbsNormPath) -> Option<AbsNormPathBuf>,
    D: ActionDirectoryRef<'a>,
{
    match entry {
        DirectoryEntry::Dir(d) => {
            if materialize_dirs_and_syms {
                fs_util::create_dir_all(&dest)?;
            }
            for (name, entry) in d.entries() {
                dest.push(name);
                if let Some((_, logical_dest)) = logical_context.as_mut() {
                    logical_dest.push(name);
                }
                materialize_recursively(
                    entry,
                    dest,
                    logical_context,
                    materialize_dirs_and_syms,
                    file_src,
                    executable_bit_override,
                )?;
                dest.pop();
                if let Some((_, logical_dest)) = logical_context.as_mut() {
                    logical_dest.pop();
                }
            }
            Ok(())
        }
        DirectoryEntry::Leaf(ActionDirectoryMember::File(_)) => {
            if let Some(src) = file_src(dest) {
                fs_util::copy(src, &dest).categorize_internal()?;
                if let Some(executable_bit_override) = executable_bit_override {
                    fs_util::set_executable(&dest, executable_bit_override)
                        .categorize_internal()?;
                }
            }
            Ok(())
        }
        DirectoryEntry::Leaf(ActionDirectoryMember::Symlink(s)) => {
            if materialize_dirs_and_syms
                && fs_util::symlink_metadata(&dest)
                    .categorize_internal()
                    .is_err()
            {
                let target = match logical_context.as_ref() {
                    Some((project_fs, logical_dest)) => {
                        project_fs.resolve_symlink_target(logical_dest, s.target())
                    }
                    None => std::path::PathBuf::from(s.target().as_str()),
                };
                fs_util::symlink(target, dest).categorize_internal()?;
            }
            Ok(())
        }
        DirectoryEntry::Leaf(ActionDirectoryMember::ExternalSymlink(s)) => {
            if materialize_dirs_and_syms
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
