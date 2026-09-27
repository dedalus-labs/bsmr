//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Supervise one privileged launcher until its isolated process population is empty.

use std::os::unix::net::UnixStream;
use std::process::Child;
use std::time::Duration;

use thiserror::Error;

use crate::identity::Identity;
use crate::identity::{self};
use crate::wait::Outcome;
use crate::wait::{self};

/// Completion failures prevent the caller from accepting action outputs.
#[derive(Debug, Error)]
pub enum Error {
    #[error("native child completion failed: {0}")]
    Child(#[from] wait::Error),
    #[error("native descendant cleanup failed: {0}")]
    Identity(#[from] identity::Error),
    #[error("native child completion failed: {child}; descendant cleanup failed: {identity}")]
    Cleanup {
        child: wait::Error,
        identity: identity::Error,
    },
}

/// Own the child across every return, while the caller retains the identity through output import.
struct Running<'a> {
    child: Child,
    identity: &'a Identity,
    cleanup_attempted: bool,
}

/// Wait for exit, cancellation, or timeout, then stop and reap all action processes.
///
/// The caller must start only its trusted credential-dropping launcher under this
/// identity. The workload must not inherit `cancel`. Retain `identity` until
/// output validation finishes. An error never authorizes publication.
pub fn run(
    child: Child,
    identity: &Identity,
    cancel: &UnixStream,
    timeout: Duration,
) -> Result<Outcome, Error> {
    let mut running = Running {
        child,
        identity,
        cleanup_attempted: false,
    };
    let observed = wait::wait(&mut running.child, Some(cancel), timeout);
    // Cleanup also runs after a kernel observation error.
    running.stop()?;
    Ok(observed?)
}

impl Running<'_> {
    /// Attempt both cleanup operations even when reaping the direct child fails.
    fn stop(&mut self) -> Result<(), Error> {
        self.cleanup_attempted = true;
        let child = self.reap();
        let identity = self.identity.drain();
        match (child, identity) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(child), Ok(())) => Err(Error::Child(child)),
            (Ok(()), Err(identity)) => Err(Error::Identity(identity)),
            (Err(child), Err(identity)) => Err(Error::Cleanup { child, identity }),
        }
    }

    /// Give the direct child one bounded stop interval before draining detached descendants.
    fn reap(&mut self) -> Result<(), wait::Error> {
        if self.child.try_wait()?.is_some() {
            return Ok(());
        }
        self.child.kill()?;
        match wait::wait(&mut self.child, None, Duration::from_secs(4))? {
            Outcome::Exited(_) => Ok(()),
            Outcome::TimedOut => Err(wait::Error::MissingExit),
            Outcome::Cancelled => unreachable!("reaping has no cancellation descriptor"),
        }
    }
}

impl Drop for Running<'_> {
    /// Keep early returns from leaving the direct child or its detached writers running.
    fn drop(&mut self) {
        if !self.cleanup_attempted
            && let Err(error) = self.stop()
        {
            eprintln!("native action cleanup incomplete: {error}");
        }
    }
}
