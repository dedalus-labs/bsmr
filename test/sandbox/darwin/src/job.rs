//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Exercise one compiler request through the native job owner and trusted launcher.

use std::fs;
use std::io::{Read, Write};
use std::path::Path;

use anyhow::{Result, ensure};
use bsmr_native::channel::Files;
use bsmr_native::job::Job;
use bsmr_native::request::Wire;
use bsmr_native::run::Outcome;
use bsmr_sandbox::{GuestAction, GuestOutput, PROTOCOL_VERSION};
use sha2::{Digest, Sha256};

/// Compile an archived source, then inspect outputs only through completed ownership.
pub(crate) fn check(parent: &Path, compiler: &Path) -> Result<()> {
    let root = tempfile::tempdir_in(parent)?;
    crate::filesystem::runtime(root.path(), compiler)?;
    let source = b"pub fn answer() -> u64 { 42 }\n";
    let mut archive = tar::Builder::new(Vec::new());
    let mut header = tar::Header::new_gnu();
    header.set_size(source.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    archive.append_data(&mut header, "input.rs", &source[..])?;
    let bytes = archive.into_inner()?;
    let mut input = tempfile::NamedTempFile::new()?;
    input.write_all(&bytes)?;
    let wire = Wire {
        environment: "qualification".into(),
        input: format!("{:x}", Sha256::digest(&bytes)),
        action: GuestAction {
            protocol: PROTOCOL_VERSION,
            arguments: [
                "/toolchain/bin/rustc",
                "--crate-type=lib",
                "--emit=metadata",
                "input.rs",
                "-o",
                "result.rmeta",
            ]
            .map(String::from)
            .to_vec(),
            environment: Default::default(),
            working_directory: "".into(),
            outputs: vec![GuestOutput::file("result.rmeta")],
            timeout_ms: Some(10_000),
        },
    };
    let mut action = tempfile::NamedTempFile::new()?;
    serde_json::to_writer(&mut action, &wire)?;
    let files = Files {
        action: fs::File::open(action.path())?,
        input: fs::File::open(input.path())?,
        output: tempfile::tempfile()?,
    };
    let job = Job::prepare(crate::identity::acquire()?, root, files, "qualification")?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    let completed = runtime.block_on(async {
        let (_client, control) = tokio::net::UnixStream::pair()?;
        job.run(std::env::current_exe()?, control).await
    })?;
    let (_, mut stderr) = completed.streams()?;
    let mut error = String::new();
    stderr.read_to_string(&mut error)?;
    ensure!(
        matches!(completed.outcome(), Outcome::Exited(status) if status.success()),
        "native job failed: {error}"
    );
    for (_, path) in completed.outputs() {
        ensure!(fs::metadata(path)?.len() > 0);
    }
    ensure!(!bsmr_native::identity::occupied(crate::identity::ID)?);
    println!("{{\"case\":\"job\",\"compiled\":true,\"empty\":true}}");
    Ok(())
}
