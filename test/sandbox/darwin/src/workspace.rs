//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Qualify the production workspace constructor with a real unprivileged action identity.

use std::fs;
use std::io::Write;
use std::path::Path;

use anyhow::{Result, ensure};
use bsmr_native::archive::Archive;
use bsmr_native::request::{Request, Wire};
use bsmr_native::workspace::Workspace;
use bsmr_sandbox::{GuestAction, GuestOutput, PROTOCOL_VERSION};
use sha2::{Digest, Sha256};

/// Use the same file snapshot, action validation and output layout as the worker.
pub(crate) fn stage(root: &Path) -> Result<()> {
    let mut archive = tar::Builder::new(Vec::new());
    for (name, content) in [
        ("input", "declared input"),
        ("output/input", "nested input"),
    ] {
        let mut header = tar::Header::new_gnu();
        header.set_size(content.len() as u64);
        header.set_mode(0o666);
        header.set_cksum();
        archive.append_data(&mut header, name, content.as_bytes())?;
    }
    let bytes = archive.into_inner()?;
    let digest = format!("{:x}", Sha256::digest(&bytes));
    let mut input = tempfile::tempfile()?;
    input.write_all(&bytes)?;
    let mut input = Archive::capture(&input, &digest)?;
    let wire = Wire {
        environment: "qualification".into(),
        input: digest,
        action: GuestAction {
            protocol: PROTOCOL_VERSION,
            arguments: vec!["/probe".into()],
            environment: Default::default(),
            working_directory: "".into(),
            outputs: vec![GuestOutput::file("output/result")],
            timeout_ms: Some(5000),
        },
    };
    let mut action = tempfile::tempfile()?;
    serde_json::to_writer(&mut action, &wire)?;
    let request = Request::read(&action, "qualification")?;
    Workspace::prepare(root, &request, &mut input)?;
    Ok(())
}

/// The action can write its output, but cannot change or unlink neighboring inputs.
pub(crate) fn check() -> Result<()> {
    ensure!(fs::read("/workspace/input")? == b"declared input");
    ensure!(fs::read("/workspace/output/input")? == b"nested input");
    for result in [
        fs::write("/workspace/input", b"changed"),
        fs::write("/workspace/output/input", b"changed"),
        fs::remove_file("/workspace/output/input"),
    ] {
        ensure!(
            matches!(result, Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied)
        );
    }
    fs::write("/workspace/output/result", b"produced output")?;
    Ok(())
}
