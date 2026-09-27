//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Own native action inputs, identity, execution and validated completion together.

use std::fs::{self, File};
use std::io;
use std::os::fd::AsRawFd;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;

use bsmr_sandbox::GuestOutput;
use thiserror::Error;
use tokio::net::UnixStream;

use crate::archive::{self, Archive};
use crate::identity::Identity;
use crate::request::{self, Request};
use crate::run::{self, Outcome};
use crate::workspace::{self, Workspace};
use bsmr_sandbox::native::files::Files;

/// The lease outlives execution and output inspection. Fields drop in this order.
pub struct Job {
    /// Drop attempts UID cleanup before removing the private filesystem.
    identity: Arc<Identity>,
    /// A supervisor-created runtime root. Never borrowed from the client.
    root: tempfile::TempDir,
    /// Validated action whose input digest was checked during preparation.
    request: Request,
    /// Input permissions and writable output parents have been established.
    workspace: Workspace,
    /// The caller-authorized result file, retained for publication after cleanup.
    output: File,
}

/// Available only after the direct child and every reserved-UID process have exited.
pub struct Completed {
    /// Keeps the root and its identity owned throughout output inspection.
    job: Job,
    /// Exit, cancellation or timeout, with cleanup confirmed in every case.
    outcome: Outcome,
}

/// No failure returns a completed action or permits publication.
#[derive(Debug, Error)]
pub enum Error {
    #[error("native request failed: {0}")]
    Request(#[from] request::Error),
    #[error("native input snapshot failed: {0}")]
    Archive(#[from] archive::Error),
    #[error("native workspace preparation failed: {0}")]
    Workspace(#[from] workspace::Error),
    #[error("native action supervision failed: {0}")]
    Run(#[from] run::Error),
    #[error("native job I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("native action serialization failed: {0}")]
    Encoding(#[from] serde_json::Error),
    #[error("native job task failed: {0}")]
    Task(#[from] tokio::task::JoinError),
}

impl Job {
    /// Bind one admitted request to verified bytes and an administrator-owned runtime root.
    ///
    /// The runtime owner supplies its checked identity, loader, `/dev/null` and
    /// fresh writable temporary directory. Its root must not contain `.bsmr` or
    /// `workspace`. The caller authenticates `files` before acquiring the lease.
    pub fn prepare(
        identity: Identity,
        root: tempfile::TempDir,
        files: Files,
        environment: &str,
    ) -> Result<Self, Error> {
        let request = Request::read(&files.action, environment)?;
        let mut input = Archive::capture(&files.input, request.input())?;
        let workspace = Workspace::prepare(root.path(), &request, &mut input)?;
        let control = root.path().join(".bsmr");
        fs::create_dir(&control)?;
        fs::set_permissions(&control, fs::Permissions::from_mode(0o700))?;
        let mut action = File::create(control.join("action.json"))?;
        serde_json::to_writer(&mut action, request.action())?;
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o755))?;
        Ok(Self {
            identity: Arc::new(identity),
            root,
            request,
            workspace,
            output: files.output,
        })
    }

    /// Execute only the trusted installed launcher, then seal completion after UID cleanup.
    ///
    /// `launcher` comes from supervisor configuration, never from the request or
    /// runtime image. It implements `enter ROOT UID LEASE_FD` through `launch::action`.
    pub async fn run(self, launcher: PathBuf, control: UnixStream) -> Result<Completed, Error> {
        // Dropping the waiter does not abandon the child, root or lease. The
        // owned task still observes disconnect/deadline and finishes cleanup.
        tokio::spawn(self.execute(launcher, control)).await?
    }

    /// Retain the lease in both supervisor and trusted launcher until credential drop.
    async fn execute(self, launcher: PathBuf, control: UnixStream) -> Result<Completed, Error> {
        let lease = self.identity.descriptor().try_clone_to_owned()?;
        let descriptor = lease.as_raw_fd();
        let mut command = tokio::process::Command::new(launcher);
        command
            .arg("enter")
            .arg(self.root.path())
            .arg(self.identity.id().to_string())
            .arg(descriptor.to_string())
            .env_clear()
            .stdin(Stdio::null())
            .stdout(File::create(self.root.path().join(".bsmr/stdout"))?)
            .stderr(File::create(self.root.path().join(".bsmr/stderr"))?)
            .kill_on_drop(true);
        // SAFETY: fcntl is async-signal-safe. `lease` owns this duplicate through
        // spawn. No descriptor number is overwritten. The launcher restores
        // CLOEXEC before executing any payload, which therefore never inherits it.
        unsafe {
            command.pre_exec(move || {
                if libc::fcntl(descriptor, libc::F_SETFD, 0) == -1 {
                    return Err(io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let child = command.spawn()?;
        drop(lease);
        let outcome = run::run(
            child,
            Arc::clone(&self.identity),
            &control,
            self.request.timeout(),
        )
        .await?;
        Ok(Completed { job: self, outcome })
    }
}

impl Completed {
    /// Return the terminal reason only after process cleanup has succeeded.
    #[must_use]
    pub fn outcome(&self) -> &Outcome {
        &self.outcome
    }

    /// Inspect declared output paths while retaining the root and identity lease.
    pub fn outputs(&self) -> impl Iterator<Item = (&GuestOutput, PathBuf)> {
        self.job
            .request
            .action()
            .outputs
            .iter()
            .map(|output| (output, self.job.workspace.path().join(&output.path)))
    }

    /// Access the caller-authorized archive after completion. Its contents remain untrusted.
    pub fn archive(&mut self) -> &mut File {
        &mut self.job.output
    }

    /// Open the completed action's captured standard streams for bounded collection.
    pub fn streams(&self) -> io::Result<(File, File)> {
        Ok((
            File::open(self.job.root.path().join(".bsmr/stdout"))?,
            File::open(self.job.root.path().join(".bsmr/stderr"))?,
        ))
    }
}
