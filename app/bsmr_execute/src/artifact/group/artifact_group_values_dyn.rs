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

use bsmr_core::fs::artifact_path_resolver::ArtifactFs;

use crate::artifact::artifact_dyn::ArtifactDyn;
use crate::artifact_value::ArtifactValue;
use crate::directory::LazyActionDirectoryBuilder;

/// This is like `ArtifactGroupValues`, but without dependency on `Artifact`.
pub trait ArtifactGroupValuesDyn: Send + Sync + 'static {
    fn iter(&self) -> Box<dyn Iterator<Item = (&dyn ArtifactDyn, &ArtifactValue)> + '_>;

    fn add_to_directory(
        &self,
        builder: &mut LazyActionDirectoryBuilder,
        artifact_fs: &ArtifactFs,
    ) -> bsmr_error::Result<()>;
}
