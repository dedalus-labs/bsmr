//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Implements the `bsmr deps` CLI for verifying, explaining, updating, and certifying DependencySets.

mod receipt;

use std::path::Path;
use std::path::PathBuf;

use bsmr_client_ctx::client_ctx::ClientCommandContext;
use bsmr_client_ctx::common::BsmrArgMatches;
use bsmr_common::argv::Argv;
use bsmr_common::argv::SanitizedArgv;
use bsmr_common::dependency_set::CandidateCatalog;
use bsmr_common::dependency_set::DEPENDENCY_LOCK_FILE;
use bsmr_common::dependency_set::DependencySetError;
use bsmr_common::dependency_set::explain;
use bsmr_common::dependency_set::load_project_state;
use bsmr_common::dependency_set::refresh_lockfiles;
use bsmr_common::dependency_set::update;
use bsmr_common::dependency_set::verify_in_root;

use self::receipt::DepsReceiptCommand;

/// Manage cross-repository DependencySet rules and exact locks.
#[derive(Debug, clap::Parser)]
#[clap(
    name = "deps",
    about = "Verify, explain, update, and certify DependencySet lockfiles"
)]
pub struct DepsCommand {
    #[clap(subcommand)]
    command: DepsSubcommand,
}

#[derive(Debug, clap::Subcommand)]
enum DepsSubcommand {
    /// Verify `bsmr.lock` against component manifests without network access.
    Verify(DepsVerifyCommand),

    /// Explain contract intersections, closures, and conflicts.
    Explain(DepsExplainCommand),

    /// Explicitly update locked components from an ordered TOML candidate catalog.
    Update(DepsUpdateCommand),

    /// Create or verify an immutable RFC 0004 release receipt.
    Receipt(DepsReceiptCommand),
}

/// Options for offline DependencySet lockfile verification.
#[derive(Debug, clap::Parser)]
struct DepsVerifyCommand {
    /// Override path to `bsmr.component.toml`.
    #[clap(long, value_name = "PATH")]
    manifest: Option<PathBuf>,

    /// Override path to `bsmr.lock`.
    #[clap(long, value_name = "PATH")]
    lock: Option<PathBuf>,

    /// Print machine-readable JSON output.
    #[clap(long)]
    json: bool,
}

/// Options for explaining DependencySet contracts and candidate compatibility.
#[derive(Debug, clap::Parser)]
struct DepsExplainCommand {
    /// Optional component name to filter contracts and closures.
    component: Option<String>,

    /// Override path to `bsmr.component.toml`.
    #[clap(long, value_name = "PATH")]
    manifest: Option<PathBuf>,

    /// Override path to `bsmr.lock`.
    #[clap(long, value_name = "PATH")]
    lock: Option<PathBuf>,

    /// Optional TOML candidate catalog to evaluate against current requirements.
    #[clap(long, value_name = "PATH")]
    catalog: Option<PathBuf>,

    /// Expected `sha256:<64hex>` digest of `--catalog`.
    #[clap(long, value_name = "SHA256", requires = "catalog")]
    catalog_digest: Option<String>,

    /// Print machine-readable JSON output.
    #[clap(long)]
    json: bool,
}

/// Options for explicitly updating one or more locked components.
#[derive(Debug, clap::Parser)]
struct DepsUpdateCommand {
    /// Component names to update. Updates all cataloged components when omitted.
    components: Vec<String>,

    /// Path to an immutable or authenticated TOML candidate catalog.
    #[clap(
        long,
        value_name = "PATH",
        required_unless_present = "refresh_lockfiles"
    )]
    catalog: Option<PathBuf>,

    /// Expected `sha256:<64hex>` digest of `--catalog`.
    #[clap(long, value_name = "SHA256", requires = "catalog")]
    catalog_digest: Option<String>,

    /// Rehash committed native ecosystem lockfiles on disk.
    #[clap(long, conflicts_with = "catalog")]
    refresh_lockfiles: bool,

    /// Override path to `bsmr.component.toml`.
    #[clap(long, value_name = "PATH")]
    manifest: Option<PathBuf>,

    /// Override path to `bsmr.lock`.
    #[clap(long, value_name = "PATH")]
    lock: Option<PathBuf>,

    /// Evaluate and print candidate decisions without writing `bsmr.lock`.
    #[clap(long)]
    dry_run: bool,

    /// Print machine-readable JSON output.
    #[clap(long)]
    json: bool,
}

impl DepsCommand {
    /// Executes a `bsmr deps` subcommand offline without starting the Bessemer daemon.
    pub fn exec(
        self,
        _matches: BsmrArgMatches<'_>,
        ctx: ClientCommandContext<'_>,
    ) -> bsmr_error::Result<()> {
        let root = resolve_root(&ctx);
        match self.command {
            DepsSubcommand::Verify(cmd) => run_verify(&root, cmd),
            DepsSubcommand::Explain(cmd) => run_explain(&root, cmd),
            DepsSubcommand::Update(cmd) => run_update(&root, cmd),
            DepsSubcommand::Receipt(cmd) => cmd.exec(&root),
        }
    }

    /// Declares that `bsmr deps` arguments carry no credentials.
    pub fn sanitize_argv(&self, argv: Argv) -> SanitizedArgv {
        argv.no_need_to_sanitize()
    }
}

/// Resolves the project root when inside a cell, or the working directory for standalone lockfiles.
fn resolve_root(ctx: &ClientCommandContext<'_>) -> PathBuf {
    match ctx.paths() {
        Ok(paths) => paths.project_root().root().as_path().to_path_buf(),
        Err(_) => ctx.working_dir.path().as_path().to_path_buf(),
    }
}

/// Runs `bsmr deps verify`.
fn run_verify(root: &Path, cmd: DepsVerifyCommand) -> bsmr_error::Result<()> {
    let manifest_path = cmd.manifest.as_deref().map(|p| root.join(p));
    let lock_path = cmd.lock.as_deref().map(|p| root.join(p));
    let (dependency_set, lock) =
        load_project_state(root, manifest_path.as_deref(), lock_path.as_deref())?;
    let verified = verify_in_root(&dependency_set, &lock, Some(root))?;

    if cmd.json {
        bsmr_client_ctx::println!("{}", serde_json::to_string_pretty(&verified)?)?;
    } else {
        bsmr_client_ctx::println!("DependencySet: {}", verified.dependency_set_digest)?;
        bsmr_client_ctx::println!("DependencyLock: {}", verified.dependency_lock_digest)?;
        bsmr_client_ctx::println!(
            "Verified {} component(s) and {} contract(s).",
            verified.component_closures.len(),
            verified.contracts.len()
        )?;
        if !verified.overlays.is_empty() {
            bsmr_client_ctx::println!(
                "Overlays (not certifiable): {}",
                verified.overlays.join(", ")
            )?;
        }
    }
    Ok(())
}

/// Runs `bsmr deps explain`.
fn run_explain(root: &Path, cmd: DepsExplainCommand) -> bsmr_error::Result<()> {
    let manifest_path = cmd.manifest.as_deref().map(|p| root.join(p));
    let lock_path = cmd.lock.as_deref().map(|p| root.join(p));
    let (dependency_set, lock) =
        load_project_state(root, manifest_path.as_deref(), lock_path.as_deref())?;
    let catalog = match cmd.catalog.as_deref() {
        Some(path) => Some(CandidateCatalog::from_path(
            &root.join(path),
            cmd.catalog_digest.as_deref(),
        )?),
        None => None,
    };

    let report = explain(
        &dependency_set,
        &lock,
        cmd.component.as_deref(),
        catalog.as_ref(),
    )?;

    if cmd.json {
        bsmr_client_ctx::println!("{}", serde_json::to_string_pretty(&report)?)?;
        return Ok(());
    }

    bsmr_client_ctx::println!("DependencySet: {}", report.dependency_set_digest)?;
    bsmr_client_ctx::println!("DependencyLock: {}", report.dependency_lock_digest)?;
    bsmr_client_ctx::println!("Status: {}", if report.valid { "valid" } else { "invalid" })?;
    if let Some(error) = &report.error {
        bsmr_client_ctx::println!("Error: {}", error)?;
    }
    for contract in &report.contracts {
        let provider_text = if contract.providers.is_empty() {
            "none".to_owned()
        } else {
            contract
                .providers
                .iter()
                .map(|(c, v)| format!("{c}={v}"))
                .collect::<Vec<_>>()
                .join(", ")
        };
        let status = match &contract.conflict {
            None => "ok".to_owned(),
            Some(conflict) => format!("CONFLICT ({conflict})"),
        };
        bsmr_client_ctx::println!(
            "  contract {}: required {}, provided by [{}] -> {}",
            contract.contract,
            contract.required_range,
            provider_text,
            status
        )?;
        for req in &contract.requirers {
            bsmr_client_ctx::println!("    required by {}: {}", req.component, req.range)?;
        }
    }
    for (component, closure) in &report.component_closures {
        bsmr_client_ctx::println!("  closure {}: {}", component, closure)?;
    }
    for decision in &report.catalog_decisions {
        let verdict = if decision.selected {
            "compatible"
        } else {
            "rejected"
        };
        match &decision.deciding_contract {
            Some(contract) => bsmr_client_ctx::println!(
                "  candidate {} release {} ({}): {} by `{}`: {}",
                decision.component,
                decision.release,
                &decision.revision[..12],
                verdict,
                contract,
                decision.reason
            )?,
            None => bsmr_client_ctx::println!(
                "  candidate {} release {} ({}): {}: {}",
                decision.component,
                decision.release,
                &decision.revision[..12],
                verdict,
                decision.reason
            )?,
        }
    }
    Ok(())
}

/// Runs `bsmr deps update`.
fn run_update(root: &Path, cmd: DepsUpdateCommand) -> bsmr_error::Result<()> {
    let manifest_path = cmd.manifest.as_deref().map(|p| root.join(p));
    let lock_path = cmd
        .lock
        .as_deref()
        .map(|p| root.join(p))
        .unwrap_or_else(|| root.join(DEPENDENCY_LOCK_FILE));
    let (dependency_set, lock) =
        load_project_state(root, manifest_path.as_deref(), Some(&lock_path))?;

    if cmd.refresh_lockfiles {
        let (updated_lock, verified, refreshed) =
            refresh_lockfiles(&dependency_set, &lock, root, &cmd.components)?;
        if !cmd.dry_run {
            updated_lock.write_atomic(&lock_path)?;
        }
        if cmd.json {
            let output = serde_json::json!({
                "refreshed_lockfiles": refreshed,
                "verification": verified,
            });
            bsmr_client_ctx::println!("{}", serde_json::to_string_pretty(&output)?)?;
        } else {
            bsmr_client_ctx::println!(
                "Refreshed {} native lockfile digest(s); DependencyLock: {}{}",
                refreshed,
                verified.dependency_lock_digest,
                if cmd.dry_run { " (dry-run)" } else { "" }
            )?;
        }
        return Ok(());
    }

    let Some(catalog_path) = cmd.catalog.as_deref() else {
        return Err(DependencySetError::InvalidCatalog(
            "`--catalog` is required unless `--refresh-lockfiles` is specified".to_owned(),
        )
        .into());
    };
    let catalog =
        CandidateCatalog::from_path(&root.join(catalog_path), cmd.catalog_digest.as_deref())?;

    let outcome = update(&dependency_set, &lock, &cmd.components, &catalog)?;
    if !cmd.dry_run {
        outcome.lock.write_atomic(&lock_path)?;
    }
    if cmd.json {
        bsmr_client_ctx::println!("{}", serde_json::to_string_pretty(&outcome)?)?;
        return Ok(());
    }

    for report in &outcome.reports {
        bsmr_client_ctx::println!(
            "Updated `{}` to release {} ({})",
            report.component,
            report.selected_release,
            &report.selected_revision[..12]
        )?;
        for decision in &report.decisions {
            let marker = if decision.selected {
                "selected"
            } else {
                "rejected"
            };
            match &decision.deciding_contract {
                Some(contract) => bsmr_client_ctx::println!(
                    "  [{}] release {} ({}) [contract: {}]: {}",
                    marker,
                    decision.release,
                    &decision.revision[..12],
                    contract,
                    decision.reason
                )?,
                None => bsmr_client_ctx::println!(
                    "  [{}] release {} ({}): {}",
                    marker,
                    decision.release,
                    &decision.revision[..12],
                    decision.reason
                )?,
            }
        }
    }
    bsmr_client_ctx::println!(
        "DependencyLock: {}{}",
        outcome.verification.dependency_lock_digest,
        if cmd.dry_run { " (dry-run)" } else { "" }
    )?;
    Ok(())
}
