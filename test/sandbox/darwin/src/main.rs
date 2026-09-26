//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Run bounded Darwin process-ownership checks without installing a service.

mod filesystem;
mod identity;
mod workload;

use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use identity::broadcast;
use identity::member;
use nix::unistd::Pid;
use workload::run;
use workload::workload;
use workload::writer;

/// Dispatch explicit probe modes. Nothing is installed or run without a mode.
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    match args.first().and_then(|arg| arg.to_str()) {
        Some("observe") if args.len() == 2 => {
            let uid = args[1].to_str().context("UID must be UTF-8")?.parse()?;
            println!(
                "{}",
                serde_json::json!({"uid": uid, "pid": member(uid)?.map(Pid::as_raw)})
            );
            Ok(())
        }
        Some("kill") if args.len() == 1 => broadcast(),
        Some("writer") if args.len() == 2 => writer(&PathBuf::from(&args[1])),
        Some("view") if args.len() == 2 => filesystem::view(&PathBuf::from(&args[1])),
        Some("normal" | "cancel") if args.len() == 2 => {
            workload(&PathBuf::from(&args[1]), args[0] == "cancel")
        }
        Some("run") if args.len() == 2 => run(&PathBuf::from(&args[1])),
        _ => bail!("usage: bsmr-darwin-check observe UID | run /private/var/tmp/NEW-DIRECTORY"),
    }
}
