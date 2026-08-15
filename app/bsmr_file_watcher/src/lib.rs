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

#![feature(used_with_arg)]

use bsmr_core::cells::paths::CellRelativePath;
use bsmr_core::fs::project_rel_path::ProjectRelativePath;

pub mod dep_files;
#[cfg(fbcode_build)]
mod edenfs;
pub mod file_watcher;
mod fs_hash_crawler;
pub mod mergebase;
mod notify;
mod stats;
mod watchman;

/// Returns true if the given path is a Watchman cookie file.
///
/// Watchman creates and deletes `.watchman-cookie-*` files as synchronization
/// markers to establish ordering barriers with the underlying filesystem
/// notification backend. These are not user source changes and should never
/// trigger DICE invalidation or rebuilds.
pub(crate) fn is_watchman_cookie(path: &ProjectRelativePath) -> bool {
    path.file_name()
        .is_some_and(|f| f.as_str().starts_with(".watchman-cookie-"))
}

/// Returns whether an ignored Git path can change a declared dynamic version.
pub(crate) fn is_vcs_identity_path(path: &CellRelativePath) -> bool {
    let path = path.as_str();
    matches!(
        path,
        ".git" | ".git/HEAD" | ".git/packed-refs" | ".git/shallow"
    ) || path.starts_with(".git/objects/")
        || path.starts_with(".git/refs/")
        || matches!(path, ".git/objects" | ".git/refs")
}

#[cfg(test)]
mod tests {
    use bsmr_core::cells::paths::CellRelativePathBuf;

    use super::is_vcs_identity_path;

    #[test]
    fn invariant_git_identity_changes_bypass_source_ignores() {
        for path in [
            ".git",
            ".git/HEAD",
            ".git/objects",
            ".git/objects/pack/demo.pack",
            ".git/packed-refs",
            ".git/refs",
            ".git/refs/tags/v1",
            ".git/shallow",
        ] {
            assert!(is_vcs_identity_path(&CellRelativePathBuf::unchecked_new(
                path.to_owned()
            )));
        }
        for path in [".git/config", ".git/index", ".git/logs/HEAD", ".github"] {
            assert!(!is_vcs_identity_path(&CellRelativePathBuf::unchecked_new(
                path.to_owned()
            )));
        }
    }
}
