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

use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;

use tracing::instrument;

use crate::bsmr::truncate_line_ending;
use crate::bsmr::utf8_output;
use crate::project_json::Sysroot;

#[derive(Debug)]
pub(crate) enum SysrootConfig {
    Sysroot(PathBuf),
    Rustup,
}

#[instrument(ret)]
pub(crate) fn resolve_rustup_sysroot() -> Result<Sysroot, anyhow::Error> {
    let mut cmd = Command::new("rustc");
    cmd.arg("--print=sysroot")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut output = utf8_output(cmd.output(), &cmd)?;
    truncate_line_ending(&mut output);
    let sysroot = PathBuf::from(output);
    let sysroot_src = sysroot
        .join("lib")
        .join("rustlib")
        .join("src")
        .join("rust")
        .join("library");

    let sysroot = Sysroot {
        sysroot,
        sysroot_src: Some(sysroot_src),
        sysroot_project: None, // rustup sysroot is not bsmrified
    };
    Ok(sysroot)
}
