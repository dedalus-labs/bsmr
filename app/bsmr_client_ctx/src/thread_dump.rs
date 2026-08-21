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

use bsmr_util::process::async_background_command;

use crate::daemon::client::connect::BsmrdProcessInfo;

pub fn thread_dump_command(
    bsmrd: &BsmrdProcessInfo<'_>,
) -> bsmr_error::Result<tokio::process::Command> {
    let pid = bsmrd.pid()?;
    let mut cmd = async_background_command("lldb");
    cmd.arg("-p")
        .arg(pid.to_string())
        .arg("--batch")
        .arg("-o")
        .arg("thread backtrace all")
        .stdin(std::process::Stdio::null());
    Ok(cmd)
}
