//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Implements `bsmr deps receipt create` and `bsmr deps receipt verify`.

use std::fs;
use std::path::Path;
use std::path::PathBuf;

use bsmr_common::dependency_set::ActionEvidence;
use bsmr_common::dependency_set::BuilderProvenance;
use bsmr_common::dependency_set::DependencySetError;
use bsmr_common::dependency_set::ReceiptArtifact;
use bsmr_common::dependency_set::ReleaseReceipt;
use bsmr_common::dependency_set::load_project_state;
use bsmr_common::dependency_set::verify_in_root;

/// Subcommands for creating and verifying immutable release receipts.
#[derive(Debug, clap::Parser)]
pub(super) struct DepsReceiptCommand {
    #[clap(subcommand)]
    command: DepsReceiptSubcommand,
}

#[derive(Debug, clap::Subcommand)]
enum DepsReceiptSubcommand {
    /// Emit an immutable release receipt for a verified lock without local overlays.
    Create(DepsReceiptCreateCommand),

    /// Verify an existing release receipt against policy and expected digests.
    Verify(DepsReceiptVerifyCommand),
}

/// Options for creating an immutable release receipt.
#[derive(Debug, clap::Parser)]
struct DepsReceiptCreateCommand {
    /// Override path to `bsmr.component.toml`.
    #[clap(long, value_name = "PATH")]
    manifest: Option<PathBuf>,

    /// Override path to `bsmr.lock`.
    #[clap(long, value_name = "PATH")]
    lock: Option<PathBuf>,

    /// Path to a JSON array of produced artifacts (`[{target, digest, image?, task_definition?, action_digest?}]`).
    #[clap(long, value_name = "PATH")]
    artifacts: PathBuf,

    /// Optional path to a JSON array of build/test action evidence.
    #[clap(long, value_name = "PATH")]
    actions: Option<PathBuf>,

    /// Full 40-character lowercase Git commit SHA for the certified source revision.
    #[clap(long, value_name = "SHA")]
    source_commit: String,

    /// Builder identity or runner principal.
    #[clap(long, value_name = "IDENTITY")]
    builder: String,

    /// Certification workflow identifier.
    #[clap(long, value_name = "WORKFLOW")]
    workflow: String,

    /// 1-based execution run attempt.
    #[clap(long, default_value_t = 1)]
    run_attempt: u64,

    /// Optional URL linking to the build execution log.
    #[clap(long, value_name = "URL")]
    build_url: Option<String>,

    /// Optional provenance or attestation references (repeatable).
    #[clap(long = "attestation", value_name = "REF")]
    attestations: Vec<String>,

    /// Destination path to write the immutable receipt (never overwrites an existing file).
    #[clap(long, value_name = "PATH")]
    output: Option<PathBuf>,
}

/// Options for verifying an immutable release receipt.
#[derive(Debug, clap::Parser)]
struct DepsReceiptVerifyCommand {
    /// Path to the release receipt JSON file.
    receipt: PathBuf,

    /// Optional expected `bsmr.dependency-set.v1` SHA-256 digest.
    #[clap(long, value_name = "SHA256")]
    expected_set_digest: Option<String>,

    /// Optional expected `bsmr.dependency-lock.v1` SHA-256 digest.
    #[clap(long, value_name = "SHA256")]
    expected_lock_digest: Option<String>,

    /// Print machine-readable JSON output.
    #[clap(long)]
    json: bool,
}

impl DepsReceiptCommand {
    /// Executes `bsmr deps receipt <create|verify>`.
    pub(super) fn exec(self, root: &Path) -> bsmr_error::Result<()> {
        match self.command {
            DepsReceiptSubcommand::Create(cmd) => run_receipt_create(root, cmd),
            DepsReceiptSubcommand::Verify(cmd) => run_receipt_verify(root, cmd),
        }
    }
}

/// Runs `bsmr deps receipt create`.
fn run_receipt_create(root: &Path, cmd: DepsReceiptCreateCommand) -> bsmr_error::Result<()> {
    let manifest_path = cmd.manifest.as_deref().map(|p| root.join(p));
    let lock_path = cmd.lock.as_deref().map(|p| root.join(p));
    let (dependency_set, lock) =
        load_project_state(root, manifest_path.as_deref(), lock_path.as_deref())?;
    verify_in_root(&dependency_set, &lock, Some(root))?;

    let artifacts_path = root.join(&cmd.artifacts);
    let artifacts_text =
        fs::read_to_string(&artifacts_path).map_err(|err| DependencySetError::Io {
            operation: "read artifacts JSON",
            path: artifacts_path.display().to_string(),
            message: err.to_string(),
        })?;
    let artifacts: Vec<ReceiptArtifact> = serde_json::from_str(&artifacts_text)
        .map_err(|err| DependencySetError::InvalidReceipt(format!("parse artifacts: {err}")))?;

    let actions: Vec<ActionEvidence> = match cmd.actions.as_deref() {
        Some(path) => {
            let resolved = root.join(path);
            let text = fs::read_to_string(&resolved).map_err(|err| DependencySetError::Io {
                operation: "read actions JSON",
                path: resolved.display().to_string(),
                message: err.to_string(),
            })?;
            serde_json::from_str(&text).map_err(|err| {
                DependencySetError::InvalidReceipt(format!("parse actions: {err}"))
            })?
        }
        None => Vec::new(),
    };

    let builder = BuilderProvenance {
        identity: cmd.builder,
        workflow: cmd.workflow,
        run_attempt: cmd.run_attempt,
        source_commit: cmd.source_commit,
        build_url: cmd.build_url,
    };
    let receipt = ReleaseReceipt::create(
        &dependency_set,
        &lock,
        artifacts,
        actions,
        builder,
        cmd.attestations,
    )?;

    if let Some(output) = cmd.output.as_deref() {
        let output_path = root.join(output);
        receipt.write_noclobber(&output_path)?;
        bsmr_client_ctx::println!(
            "Receipt: {} ({})",
            receipt.sha256_digest()?,
            output_path.display()
        )?;
    } else {
        bsmr_client_ctx::println!("{}", receipt.to_canonical_json()?.trim_end())?;
    }
    Ok(())
}

/// Runs `bsmr deps receipt verify`.
fn run_receipt_verify(root: &Path, cmd: DepsReceiptVerifyCommand) -> bsmr_error::Result<()> {
    let receipt_path = root.join(&cmd.receipt);
    let receipt = ReleaseReceipt::from_path(&receipt_path)?;
    receipt.verify(
        cmd.expected_set_digest.as_deref(),
        cmd.expected_lock_digest.as_deref(),
    )?;
    let digest = receipt.sha256_digest()?;

    if cmd.json {
        let output = serde_json::json!({
            "receipt_digest": digest,
            "dependency_set_digest": receipt.dependency_set_digest,
            "dependency_lock_digest": receipt.dependency_lock_digest,
            "source_commit": receipt.builder.source_commit,
            "artifacts": receipt.artifacts,
        });
        bsmr_client_ctx::println!("{}", serde_json::to_string_pretty(&output)?)?;
    } else {
        bsmr_client_ctx::println!("Receipt verified: {}", digest)?;
        bsmr_client_ctx::println!("DependencySet: {}", receipt.dependency_set_digest)?;
        bsmr_client_ctx::println!("DependencyLock: {}", receipt.dependency_lock_digest)?;
        bsmr_client_ctx::println!("Source commit: {}", receipt.builder.source_commit)?;
        bsmr_client_ctx::println!("Artifacts: {}", receipt.artifacts.len())?;
    }
    Ok(())
}
