//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Identifies caller-owned path packages before native source acquisition is enabled.

use anyhow::Result;
use anyhow::ensure;
use cargo::GlobalContext;
use cargo::core::Resolve;
use cargo::core::compiler::Unit;

use crate::types::Source;
use crate::types::SourceKind;

/// Export only local sources whose entire workspace is owned by the caller.
pub(crate) fn export(unit: &Unit, _resolve: &Resolve, _context: &GlobalContext) -> Result<Source> {
    let source = unit.pkg.package_id().source_id();
    ensure!(
        source.is_path(),
        "external source verification is not enabled"
    );
    Ok(Source {
        kind: SourceKind::Path,
        identity: source.as_url().to_string(),
        checksum: None,
        archive: None,
        git_revision: None,
        root: unit.pkg.root().to_owned(),
        manifest: unit.pkg.manifest_path().to_owned(),
    })
}
