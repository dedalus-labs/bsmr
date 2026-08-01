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

use bsmr_build_api::artifact_groups::ArtifactGroupValues;
use bsmr_common::file_ops::metadata::TrackedFileDigest;
use bsmr_core::fs::artifact_path_resolver::ArtifactFs;
use bsmr_directory::directory::directory::Directory;
use bsmr_directory::directory::directory_iterator::DirectoryIterator;
use bsmr_directory::directory::directory_iterator::DirectoryIteratorPathStack;
use bsmr_execute::digest_config::DigestConfig;
use bsmr_execute::directory::ActionDirectoryMember;
use bsmr_execute::directory::LazyActionDirectoryBuilder;
use bsmr_execute::execute::paths_with_digest::PathsWithDigestBlobData;
use bsmr_execute::execute::paths_with_digest::PathsWithDigestBuilder;

pub(crate) fn metadata_content(
    fs: &ArtifactFs,
    inputs: &[&ArtifactGroupValues],
    digest_config: DigestConfig,
) -> bsmr_error::Result<(PathsWithDigestBlobData, TrackedFileDigest)> {
    let mut blob_builder = PathsWithDigestBuilder::default();

    let mut builder = LazyActionDirectoryBuilder::empty();
    for &group in inputs {
        group.add_to_directory(&mut builder, fs)?;
    }
    let builder = builder.finalize()?;

    let mut walk = builder.ordered_walk_leaves();
    while let Some((path, item)) = walk.next() {
        match item {
            ActionDirectoryMember::File(metadata) => {
                blob_builder.add(path.get(), metadata.digest.data());
            }
            // Omit symlinks and let user script detect and handle symlinks in inputs.
            // Metadata will contain artifacts which are symlinked, meaning the user
            // can resolve the symlink and get the digest of the symlinked artifact.
            ActionDirectoryMember::Symlink(_) | ActionDirectoryMember::ExternalSymlink(_) => {}
        }
    }

    let blob = blob_builder.build()?;

    let digest = TrackedFileDigest::from_content(&blob.0.0, digest_config.cas_digest_config());
    Ok((blob, digest))
}
