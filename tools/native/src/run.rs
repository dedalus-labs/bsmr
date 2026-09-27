//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Reap the native child through Tokio before draining its isolated process population.

use std::io;
use std::process::ExitStatus;
use std::sync::Arc;
use std::time::Duration;

use thiserror::Error;
use tokio::net::UnixStream;
use tokio::process::Child;

use crate::identity::Identity;
use crate::identity::{self};

/// Why the remaining action processes must stop.
#[derive(Debug)]
pub enum Outcome {
    /// The direct child has exited and was reaped.
    Exited(ExitStatus),
    /// The caller sent cancellation data or closed its connection.
    Cancelled,
    /// The action deadline elapsed.
    TimedOut,
}

/// Failure to confirm that the direct child is gone.
#[derive(Debug, Error)]
pub enum StopError {
    #[error("child did not become waitable before its cleanup deadline; signal: {signal:?}")]
    Deadline { signal: Option<io::Error> },
    #[error("child wait failed: {source}; signal: {signal:?}")]
    Wait {
        source: io::Error,
        signal: Option<io::Error>,
    },
}

/// Completion failures never authorize output publication.
#[derive(Debug, Error)]
pub enum Error {
    #[error("native child observation failed: {0}")]
    Observe(#[from] io::Error),
    #[error("native child cleanup failed: {0}")]
    Child(#[source] StopError),
    #[error("native descendant cleanup failed: {0}")]
    Identity(#[source] identity::Error),
    #[error("native child cleanup failed: {child}; descendant cleanup failed: {identity}")]
    Cleanup {
        #[source]
        child: StopError,
        identity: identity::Error,
    },
    #[error("native cleanup worker failed: {0}")]
    Worker(#[from] tokio::task::JoinError),
}

/// Keep direct-child termination owned if observation is cancelled or unwinds.
struct Running(Child);

impl Drop for Running {
    /// Tokio retains the child for reaping after this best-effort stop request.
    fn drop(&mut self) {
        if let Err(error) = self.0.start_kill() {
            eprintln!("native child stop request failed: {error}");
        }
    }
}

/// Observe completion, then reap the child and require an empty reserved UID.
///
/// Start only the trusted credential-dropping launcher under this identity.
/// The payload must not inherit `cancel`. Keep another identity owner through
/// output validation. Dropping this future never produces an acceptable result.
pub async fn run(
    child: Child,
    identity: Arc<Identity>,
    cancel: &UnixStream,
    timeout: Duration,
) -> Result<Outcome, Error> {
    let mut running = Running(child);
    let observed = observe(&mut running.0, cancel, timeout).await;
    let child = stop(&mut running.0).await;
    // Cleanup retains the lease and runs outside the async I/O driver.
    let identity = tokio::task::spawn_blocking(move || identity.drain()).await?;
    match (child, identity) {
        (Ok(()), Ok(())) => Ok(observed?),
        (Err(child), Ok(())) => Err(Error::Child(child)),
        (Ok(()), Err(identity)) => Err(Error::Identity(identity)),
        (Err(child), Err(identity)) => Err(Error::Cleanup { child, identity }),
    }
}

/// Use Tokio's SIGCHLD handling rather than assuming a process event is already waitable.
async fn observe(child: &mut Child, cancel: &UnixStream, timeout: Duration) -> io::Result<Outcome> {
    tokio::select! {
        result = child.wait() => result.map(Outcome::Exited),
        result = cancel.readable() => result.map(|()| Outcome::Cancelled),
        () = tokio::time::sleep(timeout) => Ok(Outcome::TimedOut),
    }
}

/// Confirm exit even if the signal raced a process that was already leaving.
async fn stop(child: &mut Child) -> Result<(), StopError> {
    let signal = child.start_kill().err();
    match tokio::time::timeout(Duration::from_secs(4), child.wait()).await {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(source)) => Err(StopError::Wait { source, signal }),
        Err(_) => Err(StopError::Deadline { signal }),
    }
}

#[cfg(test)]
mod tests {
    use tokio::io::AsyncWriteExt;
    use tokio::process::Command;

    use super::*;

    /// A finite real process bounds cleanup even if a test fails.
    fn sleeper() -> Running {
        Running(
            Command::new("/bin/sleep")
                .arg("5")
                .kill_on_drop(true)
                .spawn()
                .unwrap(),
        )
    }

    #[tokio::test]
    async fn child_exit_preserves_its_status() {
        for _ in 0..32 {
            let mut child = Running(
                Command::new("/bin/sh")
                    .args(["-c", "exit 7"])
                    .kill_on_drop(true)
                    .spawn()
                    .unwrap(),
            );
            let (_caller, cancel) = UnixStream::pair().unwrap();
            let Outcome::Exited(status) = observe(&mut child.0, &cancel, Duration::from_secs(2))
                .await
                .unwrap()
            else {
                panic!("expected exit")
            };
            assert_eq!(status.code(), Some(7));
            stop(&mut child.0).await.unwrap();
        }
    }

    #[tokio::test]
    async fn caller_disconnect_wakes_the_supervisor() {
        let mut child = sleeper();
        let (caller, cancel) = UnixStream::pair().unwrap();
        drop(caller);
        assert!(matches!(
            observe(&mut child.0, &cancel, Duration::from_secs(2))
                .await
                .unwrap(),
            Outcome::Cancelled
        ));
        stop(&mut child.0).await.unwrap();
    }

    #[tokio::test]
    async fn cancellation_data_wakes_the_supervisor() {
        let mut child = sleeper();
        let (mut caller, cancel) = UnixStream::pair().unwrap();
        caller.write_all(&[1]).await.unwrap();
        assert!(matches!(
            observe(&mut child.0, &cancel, Duration::from_secs(2))
                .await
                .unwrap(),
            Outcome::Cancelled
        ));
        stop(&mut child.0).await.unwrap();
    }

    #[tokio::test]
    async fn deadline_wakes_the_supervisor_without_io() {
        for _ in 0..32 {
            let mut child = sleeper();
            let (_caller, cancel) = UnixStream::pair().unwrap();
            assert!(matches!(
                observe(&mut child.0, &cancel, Duration::ZERO)
                    .await
                    .unwrap(),
                Outcome::TimedOut
            ));
            stop(&mut child.0).await.unwrap();
            assert!(child.0.try_wait().unwrap().is_some());
        }
    }
}
