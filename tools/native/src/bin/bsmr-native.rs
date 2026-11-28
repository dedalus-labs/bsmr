//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Run the installed native worker or its trusted credential-dropping child.

#[cfg(target_os = "macos")]
#[derive(clap::Parser)]
enum Command {
    /// Serve the launchd-managed socket using administrator-owned configuration.
    Serve { config: std::path::PathBuf },
    /// Enter the supervisor's private root. Called only by the worker.
    Enter {
        root: std::path::PathBuf,
        uid: u32,
        descriptor: i32,
    },
    /// Exchange already-prepared action files with the installed worker.
    Exchange {
        socket: std::path::PathBuf,
        action: std::path::PathBuf,
        input: std::path::PathBuf,
        output: std::path::PathBuf,
        #[arg(long, default_value_t = 600)]
        timeout_seconds: u64,
    },
}

#[cfg(target_os = "macos")]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    use clap::Parser;
    match Command::parse() {
        Command::Serve { config } => bsmr_native::worker::serve(&config)?,
        Command::Enter {
            root,
            uid,
            descriptor,
        } => match bsmr_native::launch::action(&root, uid, descriptor)? {},
        Command::Exchange {
            socket,
            action,
            input,
            output,
            timeout_seconds,
        } => {
            let files = bsmr_native::channel::Files {
                action: std::fs::File::open(action)?,
                input: std::fs::File::open(input)?,
                output: std::fs::File::options()
                    .read(true)
                    .write(true)
                    .open(output)?,
            };
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()?;
            let status = runtime.block_on(bsmr_native::client::execute(
                &socket,
                &files,
                std::time::Duration::from_secs(timeout_seconds),
            ))?;
            println!("{}", serde_json::to_string(&status)?);
        }
    }
    Ok(())
}

#[cfg(not(target_os = "macos"))]
fn main() -> std::process::ExitCode {
    eprintln!("bsmr-native requires macOS");
    std::process::ExitCode::FAILURE
}
