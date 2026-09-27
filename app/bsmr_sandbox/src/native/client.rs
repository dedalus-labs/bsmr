//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Authenticate the privileged worker and retain cancellation ownership through its response.

use std::future::Future;
use std::io;
use std::net::Shutdown;
use std::os::fd::AsFd;
use std::path::Path;
use std::time::Duration;

use thiserror::Error;
use tokio::io::AsyncReadExt;
use tokio::io::Interest;
use tokio::net::UnixStream;

use super::files::Files;
use crate::LauncherResponse;
use crate::LauncherStatus;
use crate::MAX_TIMEOUT_MS;
use crate::PROTOCOL_VERSION;

/// A failed exchange never authorizes importing the result file.
#[derive(Debug, Error)]
pub enum Error {
    #[error("native endpoint is not owned by a root worker")]
    Peer,
    #[error("native response is oversized, incompatible, or does not confirm cleanup")]
    Response,
    #[error("native request deadline elapsed")]
    Deadline,
    #[error("native request deadline exceeds {MAX_TIMEOUT_MS} milliseconds")]
    Budget,
    #[error("native worker failed: {0}")]
    Worker(String),
    #[error("native connection failed: {0}")]
    Io(#[from] io::Error),
    #[error("native peer lookup failed: {0}")]
    Kernel(#[from] nix::errno::Errno),
    #[error("native response encoding failed: {0}")]
    Encoding(#[from] serde_json::Error),
}

/// Keep the connection through cancellation until the worker confirms cleanup.
///
/// A failed or missing acknowledgement never permits output import. Dropping
/// this future closes the connection, so the worker still receives cancellation.
pub async fn execute(
    path: &Path,
    files: &Files,
    timeout: Duration,
    cancellation: impl Future<Output = ()>,
) -> Result<LauncherStatus, Error> {
    if timeout > Duration::from_millis(MAX_TIMEOUT_MS) {
        return Err(Error::Budget);
    }
    let deadline = tokio::time::sleep(timeout);
    tokio::pin!(deadline, cancellation);
    let stream = tokio::select! {
        result = start(path, files) => result?,
        () = &mut deadline => return Ok(LauncherStatus::TimedOut),
        () = &mut cancellation => return Ok(LauncherStatus::Cancelled),
    };
    let control = std::os::unix::net::UnixStream::from(stream.as_fd().try_clone_to_owned()?);
    let response = receive(stream);
    tokio::pin!(response);
    let reason = tokio::select! {
        result = &mut response => return result,
        () = &mut deadline => LauncherStatus::TimedOut,
        () = &mut cancellation => LauncherStatus::Cancelled,
    };
    control.shutdown(Shutdown::Write)?;
    // Child reaping and UID drain each have a four-second bound.
    tokio::time::timeout(Duration::from_secs(10), response)
        .await
        .map_err(|_| Error::Deadline)??;
    Ok(reason)
}

/// Require the kernel peer to be root before disclosing any caller-owned descriptor.
async fn connect(path: &Path) -> Result<UnixStream, Error> {
    let stream = UnixStream::connect(path).await?;
    if !nix::unistd::getpeereid(&stream)?.0.is_root() {
        return Err(Error::Peer);
    }
    Ok(stream)
}

/// Complete the one-byte descriptor transfer without accepting false writable readiness.
async fn start(path: &Path, files: &Files) -> Result<UnixStream, Error> {
    let stream = connect(path).await?;
    loop {
        stream.writable().await?;
        match stream.try_io(Interest::WRITABLE, || files.send(&stream)) {
            Ok(()) => break,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Ok(stream)
}

/// Accept only a bounded response with compatible protocol and confirmed cleanup.
async fn receive(stream: UnixStream) -> Result<LauncherStatus, Error> {
    let mut bytes = Vec::new();
    stream.take(64 * 1024 + 1).read_to_end(&mut bytes).await?;
    if bytes.len() > 64 * 1024 {
        return Err(Error::Response);
    }
    let response: LauncherResponse = serde_json::from_slice(&bytes)?;
    if response.protocol != PROTOCOL_VERSION {
        return Err(Error::Response);
    }
    if response.status == LauncherStatus::Failed {
        return Err(Error::Worker(response.error.ok_or(Error::Response)?));
    }
    if !response.cleanup_complete || response.error.is_some() {
        return Err(Error::Response);
    }
    Ok(response.status)
}
