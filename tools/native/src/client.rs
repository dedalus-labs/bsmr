//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Authenticate the privileged worker and retain cancellation ownership through its response.

use std::io;
use std::path::Path;
use std::time::Duration;

use bsmr_sandbox::{LauncherResponse, LauncherStatus, MAX_TIMEOUT_MS, PROTOCOL_VERSION};
use thiserror::Error;
use tokio::io::{AsyncReadExt, Interest};
use tokio::net::UnixStream;

use crate::channel::Files;

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

/// Bound the entire exchange. Dropping or timing out this future closes the cancellation socket.
pub async fn execute(
    path: &Path,
    files: &Files,
    timeout: Duration,
) -> Result<LauncherStatus, Error> {
    if timeout > Duration::from_millis(MAX_TIMEOUT_MS) {
        return Err(Error::Budget);
    }
    tokio::time::timeout(timeout, exchange(path, files))
        .await
        .map_err(|_| Error::Deadline)?
}

/// Require the kernel peer to be root before disclosing any caller-owned descriptor.
async fn connect(path: &Path) -> Result<UnixStream, Error> {
    let stream = UnixStream::connect(path).await?;
    if !nix::unistd::getpeereid(&stream)?.0.is_root() {
        return Err(Error::Peer);
    }
    Ok(stream)
}

/// Send once and read a bounded terminal response, clearing false writable readiness hints.
async fn exchange(path: &Path, files: &Files) -> Result<LauncherStatus, Error> {
    let stream = connect(path).await?;
    loop {
        stream.writable().await?;
        match stream.try_io(Interest::WRITABLE, || files.send(&stream)) {
            Ok(()) => break,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => continue,
            Err(error) => return Err(error.into()),
        }
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn ordinary_processes_cannot_impersonate_the_worker() {
        if nix::unistd::Uid::current().is_root() {
            return;
        }
        let root = tempfile::tempdir().unwrap();
        let socket = root.path().join("worker.sock");
        let _listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
        assert!(matches!(connect(&socket).await, Err(Error::Peer)));
    }
}
