//===----------------------------------------------------------------------===//
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

use std::sync::LazyLock;

use allocative::Allocative;
use bsmr_core::bsmr_env;
use bsmr_core::fs::project::ProjectRoot;
use bsmr_core::fs::project_rel_path::ProjectRelativePathBuf;
use bsmr_error::BuckErrorContext;
use bsmr_error::internal_error;
use bsmr_fs::error::IoResultExt;
use bsmr_fs::fs_util;
use bsmr_fs::paths::abs_norm_path::AbsNormPath;
use bsmr_fs::paths::abs_norm_path::AbsNormPathBuf;
use bsmr_fs::paths::abs_path::AbsPathBuf;
use bsmr_fs::paths::file_name::FileName;
use bsmr_fs::paths::file_name::FileNameBuf;
use bsmr_fs::working_dir::AbsWorkingDir;

use crate::invocation_paths::InvocationPaths;
use crate::invocation_paths_result::InvocationPathsResult;

#[derive(Debug, bsmr_error::Error)]
enum BsmrCliError {
    #[error(
        "Couldn't find a Bessemer project root for directory `{}`. Expected to find a .bsmr file.", _0.path().display()
    )]
    #[bsmr(tag = NoBsmrRoot)]
    NoBsmrRoot(AbsWorkingDir),
}

#[derive(Clone, Allocative)]
pub struct InvocationRoots {
    pub project_root: ProjectRoot,
    pub cwd: ProjectRelativePathBuf,
}

impl InvocationRoots {
    pub fn common_buckd_dir(&self) -> bsmr_error::Result<AbsNormPathBuf> {
        Ok(home_buck_dir()?.join(FileName::unchecked_new("buckd")))
    }

    pub fn paranoid_info_path(&self) -> bsmr_error::Result<AbsPathBuf> {
        // Used in tests
        if let Some(p) = bsmr_env!("BSMR_PARANOID_PATH")? {
            return AbsPathBuf::try_from(p.to_owned());
        }

        Ok(self
            .common_buckd_dir()?
            .join(FileName::new("paranoid.info")?)
            .into_abs_path_buf())
    }
}

/// Finds the project root.
///
/// The nearest `.bsmr` with `[project] root = .` is both config and root marker.
fn get_roots(from: &AbsWorkingDir) -> bsmr_error::Result<Option<InvocationRoots>> {
    let home_dir = dirs::home_dir();
    for curr in from.path().ancestors() {
        // Never treat a user's home-level configuration as a project.
        if home_dir.as_ref().is_some_and(|home| home == curr.as_path()) {
            break;
        }
        let project_file = curr.join(FileName::unchecked_new(".bsmr"));
        if fs_util::try_exists(&project_file)?
            && has_project_marker(&fs_util::read_to_string(&project_file).categorize_internal()?)
        {
            let rel_cwd = from
                .path()
                .strip_prefix(curr)
                .expect("ancestor must prefix working directory")
                .into_owned();
            return Ok(Some(InvocationRoots {
                project_root: ProjectRoot::new_unchecked(curr.to_owned()),
                cwd: rel_cwd.into(),
            }));
        }
    }
    Ok(None)
}

/// Recognizes the explicit root marker embedded in the canonical project file.
fn has_project_marker(source: &str) -> bool {
    let mut in_project_section = false;
    for line in source.lines() {
        let line = line.split(['#', ';']).next().unwrap_or("").trim();
        if line.starts_with('[') && line.ends_with(']') {
            in_project_section = line == "[project]";
            continue;
        }
        if in_project_section
            && line
                .split_once('=')
                .is_some_and(|(key, value)| key.trim() == "root" && value.trim() == ".")
        {
            return true;
        }
    }
    false
}

pub fn find_invocation_roots(from: &AbsWorkingDir) -> bsmr_error::Result<InvocationRoots> {
    get_roots(from)?.ok_or_else(|| BsmrCliError::NoBsmrRoot(from.to_owned()).into())
}

pub fn get_invocation_paths_result(
    from: &AbsWorkingDir,
    isolation: FileNameBuf,
) -> InvocationPathsResult {
    match get_roots(from) {
        Ok(Some(roots)) => InvocationPathsResult::Paths(InvocationPaths { roots, isolation }),
        Ok(None) => {
            InvocationPathsResult::OutsideOfRepo(BsmrCliError::NoBsmrRoot(from.to_owned()).into())
        }
        Err(e) => InvocationPathsResult::OtherError(e),
    }
}

/// `~/.buck`.
/// TODO(cjhopman): We currently place all buckd info into a directory owned by the user.
/// This is broken when multiple users try to share the same checkout.
///
/// **This is different than the behavior of buck1.**
///
/// In buck1, the buck daemon is shared across users. Due to the fact that `buck run`
/// will run whatever command is returned by the daemon, buck1 has a privilege escalation
/// vulnerability.
///
/// There's a couple ways we could resolve this:
///
/// 1. Use a shared .buckd information directory and have the client verify the identity of
///    the server before doing anything with it. If the identity is different, kill it and
///    start a new one.
///
/// 2. Keep user-owned .buckd directory, use some other mechanism to move ownership of
///    output directories between different buckd instances.
pub(crate) fn home_buck_dir() -> bsmr_error::Result<&'static AbsNormPath> {
    fn find_dir() -> bsmr_error::Result<AbsNormPathBuf> {
        let home = dirs::home_dir()
            .ok_or_else(|| internal_error!("Expected a HOME directory to be available"))?;
        let home =
            AbsNormPathBuf::new(home).buck_error_context("Expected an absolute HOME directory")?;
        Ok(home.join(FileName::new(".buck")?))
    }

    static DIR: LazyLock<bsmr_error::Result<AbsNormPathBuf>> = LazyLock::new(find_dir);

    Ok(LazyLock::force(&DIR).as_ref().map_err(dupe::Dupe::dupe)?)
}

#[cfg(test)]
mod tests {
    use bsmr_fs::fs_util::uncategorized as fs_util;

    use super::*;

    /// A working directory without its own config inherits the nearest project root.
    #[test]
    fn nearest_unified_config_defines_project_root() -> bsmr_error::Result<()> {
        let temp = tempfile::tempdir()?;
        let project = AbsNormPathBuf::new(temp.path().canonicalize()?)?;
        let nested = project.join(FileName::unchecked_new("nested"));
        fs_util::create_dir_all(&nested)?;
        fs_util::write(
            project.join(FileName::unchecked_new(".bsmr")),
            "[project]\nroot = .\n",
        )?;

        let roots = find_invocation_roots(&AbsWorkingDir::unchecked_new(nested))?;

        assert_eq!(roots.project_root.root(), &*project);
        assert_eq!(
            roots.cwd,
            ProjectRelativePathBuf::unchecked_new("nested".to_owned())
        );
        Ok(())
    }

    /// A nested cell config cannot silently replace the enclosing project root.
    #[test]
    fn cell_config_without_project_marker_is_not_a_root() -> bsmr_error::Result<()> {
        let temp = tempfile::tempdir()?;
        let project = AbsNormPathBuf::new(temp.path().canonicalize()?)?;
        let cell = project.join(FileName::unchecked_new("cell"));
        fs_util::create_dir_all(&cell)?;
        fs_util::write(
            project.join(FileName::unchecked_new(".bsmr")),
            "[project]\nroot = .\n",
        )?;
        fs_util::write(
            cell.join(FileName::unchecked_new(".bsmr")),
            "[cell]\nname = nested\n",
        )?;

        let roots = find_invocation_roots(&AbsWorkingDir::unchecked_new(cell))?;

        assert_eq!(roots.project_root.root(), &*project);
        assert_eq!(
            roots.cwd,
            ProjectRelativePathBuf::unchecked_new("cell".to_owned())
        );
        Ok(())
    }

    /// Legacy marker files must not silently create a second project-discovery mode.
    #[test]
    fn legacy_root_files_are_not_project_markers() -> bsmr_error::Result<()> {
        let temp = tempfile::tempdir()?;
        let project = AbsNormPathBuf::new(temp.path().canonicalize()?)?;
        fs_util::write(
            project.join(FileName::unchecked_new(".bsmrconfig")),
            "[cells]\nroot = .\n",
        )?;
        fs_util::write(project.join(FileName::unchecked_new(".bsmrroot")), "")?;

        let result = find_invocation_roots(&AbsWorkingDir::unchecked_new(project));

        assert!(result.is_err());
        Ok(())
    }
}
