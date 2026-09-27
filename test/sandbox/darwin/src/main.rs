//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Run bounded Darwin process-ownership checks without installing a service.

mod compiler;
mod filesystem;
mod identity;
mod image;
mod job;
mod system;
mod workload;
mod workspace;

use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use bsmr_native::identity::occupied;
use workload::run;
use workload::workload;
use workload::writer;

/// Dispatch explicit probe modes. Nothing is installed or run without a mode.
fn main() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    match args.first().and_then(|arg| arg.to_str()) {
        Some("copy") if args.len() == 3 => {
            std::fs::copy(&args[1], &args[2])?;
            Ok(())
        }
        Some("wait") if args.len() == 1 => {
            std::thread::sleep(std::time::Duration::from_secs(60));
            Ok(())
        }
        Some("seed") if args.len() == 3 => {
            filesystem::seed(&PathBuf::from(&args[1]), &PathBuf::from(&args[2]))
        }
        Some("enter") if args.len() == 4 => {
            let uid = args[2].to_str().context("UID must be UTF-8")?.parse()?;
            let fd = args[3]
                .to_str()
                .context("descriptor must be UTF-8")?
                .parse()?;
            match bsmr_native::launch::action(&PathBuf::from(&args[1]), uid, fd)? {}
        }
        Some("prepare") if args.len() == 2 => compiler::record(&PathBuf::from(&args[1])),
        Some("observe") if args.len() == 2 => {
            let uid = args[1].to_str().context("UID must be UTF-8")?.parse()?;
            println!(
                "{}",
                serde_json::json!({"uid": uid, "occupied": occupied(uid)?})
            );
            Ok(())
        }
        Some("writer") if args.len() == 2 => writer(&PathBuf::from(&args[1])),
        Some("view") if args.len() == 4 => filesystem::view(
            &PathBuf::from(&args[1]),
            &PathBuf::from(&args[2]),
            args[3].to_str().context("address must be UTF-8")?.parse()?,
        ),
        Some("normal" | "cancel") if args.len() == 2 => {
            workload(&PathBuf::from(&args[1]), args[0] == "cancel")
        }
        Some("run") if args.len() == 3 => run(&PathBuf::from(&args[1]), &PathBuf::from(&args[2])),
        _ => bail!(
            "usage: bsmr-darwin-check observe UID | prepare FILE | run /private/var/tmp/NEW-DIRECTORY FILE"
        ),
    }
}
