//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Selects the authoritative source used to infer one package's build graph.

use allocative::Allocative;
use bsmr_fs::paths::file_name::FileNameBuf;
use pagable::Pagable;

use crate::file_ops::metadata::SimpleDirEntry;
use crate::find_buildfile::find_buildfile;

/// The authoritative source used to define a package's build graph.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Allocative, Pagable)]
pub enum PackageBuildSource {
    /// An explicitly authored Starlark build file.
    Starlark,
    /// A supported native ecosystem manifest interpreted by BSMR.
    Native,
}

/// Selects an explicit build file or, at a requested package root, a native manifest.
pub(crate) fn find_build_source(
    buildfile_candidates: &[FileNameBuf],
    dir_listing: &[SimpleDirEntry],
    allow_native: bool,
) -> Option<(FileNameBuf, PackageBuildSource)> {
    if let Some(buildfile) = find_buildfile(buildfile_candidates, dir_listing) {
        return Some((buildfile.to_owned(), PackageBuildSource::Starlark));
    }
    if !allow_native {
        return None;
    }
    ["pyproject.toml", "Cargo.toml", "package.json"]
        .into_iter()
        .find_map(|name| {
            dir_listing
                .iter()
                .any(|entry| entry.file_name == name)
                .then(|| (FileNameBuf::unchecked_new(name), PackageBuildSource::Native))
        })
}

/// Returns whether a directory is the root of a standard Python virtual environment.
pub(crate) fn is_python_virtual_environment(dir_listing: &[SimpleDirEntry]) -> bool {
    dir_listing
        .iter()
        .any(|entry| entry.file_name == "pyvenv.cfg" && entry.file_type.is_file())
}

#[cfg(test)]
mod tests {
    use bsmr_fs::paths::file_name::FileNameBuf;

    use super::PackageBuildSource;
    use super::find_build_source;
    use super::is_python_virtual_environment;
    use crate::file_ops::metadata::FileType;
    use crate::file_ops::metadata::SimpleDirEntry;

    /// Creates a deterministic file-only directory listing.
    fn listing(names: &[&str]) -> Vec<SimpleDirEntry> {
        names
            .iter()
            .map(|name| SimpleDirEntry {
                file_name: FileNameBuf::unchecked_new(*name),
                file_type: FileType::File,
            })
            .collect()
    }

    #[test]
    fn invariant_explicit_build_file_wins_over_native_manifests() {
        let candidates = [FileNameBuf::unchecked_new("BUILD.bsmr")];
        let source = find_build_source(
            &candidates,
            &listing(&["Cargo.toml", "package.json", "BUILD.bsmr"]),
            true,
        );

        assert_eq!(
            source,
            Some((candidates[0].clone(), PackageBuildSource::Starlark))
        );
    }

    #[test]
    fn invariant_package_json_defines_a_requested_package() {
        let source = find_build_source(&[], &listing(&["package.json"]), true);

        assert_eq!(
            source,
            Some((
                FileNameBuf::unchecked_new("package.json"),
                PackageBuildSource::Native,
            ))
        );
    }

    #[test]
    fn invariant_cargo_toml_defines_a_requested_package() {
        let source = find_build_source(&[], &listing(&["Cargo.toml"]), true);

        assert_eq!(
            source,
            Some((
                FileNameBuf::unchecked_new("Cargo.toml"),
                PackageBuildSource::Native,
            ))
        );
    }

    #[test]
    fn invariant_pyproject_toml_defines_a_requested_package() {
        let source = find_build_source(&[], &listing(&["pyproject.toml"]), true);

        assert_eq!(
            source,
            Some((
                FileNameBuf::unchecked_new("pyproject.toml"),
                PackageBuildSource::Native,
            ))
        );
    }

    #[test]
    fn invariant_native_manifests_do_not_create_recursive_package_boundaries() {
        assert_eq!(
            find_build_source(&[], &listing(&["Cargo.toml", "package.json"]), false),
            None
        );
    }

    #[test]
    fn invariant_pyvenv_cfg_identifies_virtual_environment_roots() {
        assert!(is_python_virtual_environment(&listing(&["pyvenv.cfg"])));
        assert!(!is_python_virtual_environment(&listing(&[
            "pyproject.toml"
        ])));
    }
}
