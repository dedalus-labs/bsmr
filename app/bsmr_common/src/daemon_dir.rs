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

use bsmr_fs::paths::abs_norm_path::AbsNormPathBuf;
use bsmr_fs::paths::file_name::FileName;

/// `~/.bsmr/bsmrd/repo-path` directory.
#[derive(Debug, Clone, derive_more::Display)]
#[display("{}", path.display())]
pub struct DaemonDir {
    pub path: AbsNormPathBuf,
}

impl DaemonDir {
    /// Path to `bsmrd.info` file.
    pub fn bsmrd_info(&self) -> AbsNormPathBuf {
        self.path.join(FileName::new("bsmrd.info").unwrap())
    }

    /// Path to `bsmrd.stdout` file.
    pub fn bsmrd_stdout(&self) -> AbsNormPathBuf {
        self.path.join(FileName::new("bsmrd.stdout").unwrap())
    }

    /// Path to `bsmrd.stderr` file.
    pub fn bsmrd_stderr(&self) -> AbsNormPathBuf {
        self.path.join(FileName::new("bsmrd.stderr").unwrap())
    }

    /// Path to `bsmrd.pid` file.
    pub fn bsmrd_pid(&self) -> AbsNormPathBuf {
        self.path.join(FileName::new("bsmrd.pid").unwrap())
    }

    pub fn bsmrd_error_log(&self) -> AbsNormPathBuf {
        self.path.join(FileName::new("bsmrd.error.log").unwrap())
    }
}
