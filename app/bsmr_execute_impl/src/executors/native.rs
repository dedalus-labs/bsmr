//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Connect native actions to the shared protocol, materializer and result validator.

use std::fs::File;
use std::io::Read;
use std::io::Seek;
use std::io::SeekFrom;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use bsmr_common::execution::NATIVE_PROFILE;
use bsmr_common::liveliness_observer::LivelinessObserver;
use bsmr_execute::digest_config::DigestConfig;
use bsmr_execute::execute::prepared::PreparedAction;
use bsmr_execute::execute::request::CommandExecutionRequest;
use bsmr_execute_local::CommandResult;
use bsmr_execute_local::GatherOutputStatus;
use bsmr_sandbox::GuestAction;
use bsmr_sandbox::LauncherStatus;
use bsmr_sandbox::MAX_TIMEOUT_MS;
use bsmr_sandbox::PROTOCOL_VERSION;
use bsmr_sandbox::native::files::Files;
use bsmr_sandbox::native::protocol::Info;
use bsmr_sandbox::native::protocol::Wire;
use tokio::sync::OwnedSemaphorePermit;
use tokio::sync::Semaphore;

use super::firecracker;

/// Bind the selected runtime to both action keys and every worker request.
pub struct NativeExecutor {
    socket: PathBuf,
    environment: String,
    slots: Arc<Semaphore>,
}

/// Retain the exact opened inputs and output declarations until worker cleanup completes.
pub(crate) struct NativeAction {
    action: GuestAction,
    files: Files,
}

#[derive(Debug, bsmr_error::Error)]
#[bsmr(tag = Input)]
enum Error {
    #[error("native worker metadata is invalid or incompatible: {0:?}")]
    Info(PathBuf),
    #[error("native worker failed: {0}")]
    Worker(String),
    #[error("native worker capacity is closed")]
    Closed,
}

impl NativeExecutor {
    /// Load only root-owned metadata before constructing the cache-key platform.
    pub fn new(socket: &Path) -> bsmr_error::Result<Self> {
        let path = socket.with_extension("json");
        bsmr_sandbox::verify_root_owned_chain(&path)
            .map_err(|error| Error::Worker(error.to_string()))?;
        let mut bytes = Vec::new();
        File::open(&path)?
            .take(64 * 1024 + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() > 64 * 1024 {
            return Err(Error::Info(path).into());
        }
        let info: Info = serde_json::from_slice(&bytes)?;
        if info.protocol != PROTOCOL_VERSION
            || info.slots != 1
            || info.environment.len() != 64
            || !info
                .environment
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(Error::Info(path).into());
        }
        Ok(Self {
            socket: socket.to_owned(),
            environment: info.environment,
            slots: Arc::new(Semaphore::new(info.slots)),
        })
    }

    /// Match the installed worker and its verified runtime before any cache lookup.
    pub fn platform(&self) -> remote_execution::Platform {
        remote_execution::Platform {
            properties: [
                ("bsmr.sandbox.backend", "native"),
                ("bsmr.sandbox.environment", &self.environment),
                ("bsmr.sandbox.profile", NATIVE_PROFILE),
            ]
            .into_iter()
            .map(|(name, value)| remote_execution::Property {
                name: name.into(),
                value: value.into(),
            })
            .collect(),
        }
    }

    /// Reserve capacity before creating archives. Cancellation never leaves a queued owner.
    pub(crate) async fn claim(
        &self,
        liveliness: &dyn LivelinessObserver,
    ) -> bsmr_error::Result<Option<OwnedSemaphorePermit>> {
        tokio::select! {
            result = Arc::clone(&self.slots).acquire_owned() => Ok(Some(result.map_err(|_| Error::Closed)?)),
            () = liveliness.while_alive() => Ok(None),
        }
    }

    /// Reuse the canonical command and declared-input archive writer.
    pub(crate) fn prepare(
        &self,
        prepared: &PreparedAction,
        request: &CommandExecutionRequest,
        root: &Path,
        digest: DigestConfig,
    ) -> bsmr_error::Result<NativeAction> {
        firecracker::validate_action_policy(prepared, request)?;
        let command = firecracker::decode_re_command(prepared, digest)?;
        let action = firecracker::sandbox_action(&command, request)?;
        let mut input = tempfile::NamedTempFile::new()?;
        firecracker::write_input_archive(
            input.as_file_mut(),
            root,
            request.paths().input_directory(),
            digest,
        )?;
        let mut description = tempfile::NamedTempFile::new()?;
        let wire = Wire {
            environment: self.environment.clone(),
            input: bsmr_sandbox::sha256_file(input.path())
                .map_err(|error| Error::Worker(error.to_string()))?,
            action: action.clone(),
        };
        serde_json::to_writer(&mut description, &wire)?;
        let files = Files {
            action: File::open(description.path())?,
            input: File::open(input.path())?,
            output: tempfile::tempfile()?,
        };
        Ok(NativeAction { action, files })
    }

    /// Keep cancellation ownership until the authenticated worker confirms cleanup.
    pub(crate) async fn run(
        &self,
        action: &NativeAction,
        liveliness: &dyn LivelinessObserver,
    ) -> bsmr_error::Result<LauncherStatus> {
        bsmr_sandbox::native::client::execute(
            &self.socket,
            &action.files,
            action.timeout(),
            liveliness.while_alive(),
        )
        .await
        .map_err(|error| Error::Worker(error.to_string()).into())
    }
}

impl NativeAction {
    fn timeout(&self) -> Duration {
        Duration::from_millis(self.action.timeout_ms.unwrap_or(MAX_TIMEOUT_MS))
    }

    /// Apply the existing untrusted-output validator and materializer after acknowledged cleanup.
    pub(crate) fn result(
        mut self,
        status: LauncherStatus,
        root: &Path,
    ) -> bsmr_error::Result<CommandResult> {
        let status = match status {
            LauncherStatus::Completed => {
                self.files.output.seek(SeekFrom::Start(0))?;
                let staging = tempfile::Builder::new()
                    .prefix(".bsmr-native-")
                    .tempdir_in(root)?;
                let result = firecracker::extract_guest_outputs(
                    &mut self.files.output,
                    staging.path(),
                    &self.action.outputs,
                )?;
                firecracker::import_outputs(staging.path(), root, &self.action.outputs)?;
                let status = if result.timed_out {
                    GatherOutputStatus::TimedOut(self.timeout())
                } else {
                    GatherOutputStatus::Finished {
                        exit_code: result.exit_code,
                        execution_stats: None,
                    }
                };
                return Ok(CommandResult {
                    status,
                    stdout: result.stdout,
                    stderr: result.stderr,
                    cgroup_result: None,
                    orphan_processes: Vec::new(),
                });
            }
            LauncherStatus::TimedOut => GatherOutputStatus::TimedOut(self.timeout()),
            LauncherStatus::Cancelled => GatherOutputStatus::Cancelled,
            LauncherStatus::Failed => {
                return Err(Error::Worker("worker returned an unvalidated failure".into()).into());
            }
        };
        Ok(CommandResult {
            status,
            stdout: Vec::new(),
            stderr: Vec::new(),
            cgroup_result: None,
            orphan_processes: Vec::new(),
        })
    }
}
