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

use std::sync::Arc;

use allocative::Allocative;
use bsmr_core::package::package_relative_path::PackageRelativePath;
use bsmr_fs::paths::file_name::FileName;
use bsmr_fs::paths::file_name::FileNameBuf;
use bsmr_util::arc_str::ArcS;
use dupe::Dupe;
use pagable::Pagable;
use starlark_map::sorted_set::SortedSet;
use starlark_map::sorted_vec::SortedVec;

use crate::package_listing::PackageBuildSource;
use crate::package_listing::file_listing::PackageFileListing;

#[derive(Clone, Dupe, Eq, PartialEq, Debug, Allocative, Pagable)]
pub struct PackageListing {
    listing: Arc<PackageListingData>,
}

#[derive(Eq, PartialEq, Debug, Allocative, Pagable)]
struct PackageListingData {
    files: PackageFileListing,
    directories: SortedSet<ArcS<PackageRelativePath>>,
    subpackages: SortedVec<ArcS<PackageRelativePath>>,
    buildfile: FileNameBuf,
    build_source: PackageBuildSource,
}

impl PackageListing {
    pub(crate) fn new(
        files: SortedSet<ArcS<PackageRelativePath>>,
        directories: SortedSet<ArcS<PackageRelativePath>>,
        subpackages: SortedVec<ArcS<PackageRelativePath>>,
        buildfile: FileNameBuf,
        build_source: PackageBuildSource,
    ) -> Self {
        Self {
            listing: Arc::new(PackageListingData {
                files: PackageFileListing { files },
                directories,
                subpackages,
                buildfile,
                build_source,
            }),
        }
    }

    pub fn empty(buildfile: FileNameBuf) -> Self {
        Self::new(
            SortedSet::new(),
            SortedSet::new(),
            SortedVec::new(),
            buildfile,
            PackageBuildSource::Starlark,
        )
    }

    pub fn files(&self) -> &PackageFileListing {
        &self.listing.files
    }

    pub fn get_file(&self, file: &PackageRelativePath) -> Option<ArcS<PackageRelativePath>> {
        self.listing.files.get_file(file)
    }

    pub fn get_dir(&self, dir: &PackageRelativePath) -> Option<ArcS<PackageRelativePath>> {
        // Empty paths must refer to a directory, since the whole thing is rooted
        // at a directory. But empty paths are not explicitly added to the `directories` variable,
        // so handle them specially.
        if dir.is_empty() {
            Some(ArcS::from(PackageRelativePath::empty()))
        } else {
            self.listing.directories.get(dir).map(|x| x.dupe())
        }
    }

    pub fn files_within<'a>(
        &'a self,
        dir: &PackageRelativePath,
    ) -> impl Iterator<Item = &'a ArcS<PackageRelativePath>> + use<'a> {
        self.listing.files.files_within(dir)
    }

    pub fn subpackages_within<'a>(
        &'a self,
        dir: &'a PackageRelativePath,
    ) -> impl Iterator<Item = &'a PackageRelativePath> + 'a {
        self.listing
            .subpackages
            .iter()
            .map(|x| x.as_ref())
            .filter(move |x: &&PackageRelativePath| x.starts_with(dir))
    }

    pub fn buildfile(&self) -> &FileName {
        &self.listing.buildfile
    }

    /// Returns whether this package is explicit Starlark or native-manifest inferred.
    #[must_use]
    pub fn build_source(&self) -> PackageBuildSource {
        self.listing.build_source
    }
}

pub mod testing {
    use bsmr_core::package::package_relative_path::PackageRelativePathBuf;
    use bsmr_fs::paths::file_name::FileNameBuf;
    use starlark_map::sorted_set::SortedSet;
    use starlark_map::sorted_vec::SortedVec;

    use crate::package_listing::listing::PackageListing;

    pub trait PackageListingExt {
        fn testing_empty() -> Self;
        fn testing_files(files: &[&str]) -> Self;
        fn testing_new(files: &[&str], buildfile: &str) -> Self;
    }

    impl PackageListingExt for PackageListing {
        fn testing_empty() -> Self {
            Self::testing_files(&[])
        }

        fn testing_files(files: &[&str]) -> Self {
            Self::testing_new(files, "BUCK")
        }

        fn testing_new(files: &[&str], buildfile: &str) -> Self {
            let files = files.iter().map(|f| {
                PackageRelativePathBuf::try_from((*f).to_owned())
                    .unwrap()
                    .to_arc()
            });
            PackageListing::new(
                SortedSet::from_iter(files),
                SortedSet::new(),
                SortedVec::new(),
                FileNameBuf::unchecked_new(buildfile),
                crate::package_listing::PackageBuildSource::Starlark,
            )
        }
    }
}
