//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Wait for process exit, caller cancellation, or a deadline through one kernel queue.

use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::process::Child;
use std::process::ExitStatus;
use std::time::Duration;
use std::time::Instant;

use nix::errno::Errno;
use nix::sys::event::EvFlags;
use nix::sys::event::EventFilter;
use nix::sys::event::FilterFlag;
use nix::sys::event::KEvent;
use nix::sys::event::Kqueue;
use thiserror::Error;

/// Why the supervisor must stop the remaining process population.
#[derive(Debug)]
pub enum Outcome {
    /// The direct child has exited and was reaped.
    Exited(ExitStatus),
    /// The caller sent cancellation data or closed its connection.
    Cancelled,
    /// The absolute deadline elapsed.
    TimedOut,
}

/// Kernel and child-observation failures never count as successful completion.
#[derive(Debug, Error)]
pub enum Error {
    #[error("process event operation failed: {0}")]
    Kernel(#[from] Errno),
    #[error("child status operation failed: {0}")]
    Child(#[from] std::io::Error),
    #[error("process event did not provide a waitable exit")]
    MissingExit,
    #[error("unexpected native process event")]
    Event,
    #[error("native action timeout cannot be represented")]
    Deadline,
}

/// Observe a child without polling. The caller still owns all descendant cleanup.
///
/// `cancel` must be a private connection that the workload cannot inherit.
/// Both readable data and EOF request cancellation. Returned child exits have
/// been reaped. Errors, cancellation, and timeouts leave termination to the owner.
pub fn wait(
    child: &mut Child,
    cancel: Option<&UnixStream>,
    timeout: Duration,
) -> Result<Outcome, Error> {
    let deadline = Instant::now().checked_add(timeout).ok_or(Error::Deadline)?;
    let queue = Kqueue::new()?;
    let mut changes = vec![KEvent::new(
        child.id() as usize,
        EventFilter::EVFILT_PROC,
        EvFlags::EV_ADD,
        FilterFlag::NOTE_EXIT,
        0,
        0,
    )];
    if let Some(cancel) = cancel {
        changes.push(KEvent::new(
            cancel.as_raw_fd() as usize,
            EventFilter::EVFILT_READ,
            EvFlags::EV_ADD,
            FilterFlag::empty(),
            0,
            0,
        ));
    }
    let mut pending = changes.as_slice();
    let mut events = [changes[0]; 2];
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(Outcome::Exited(status));
        }
        let remaining = deadline.saturating_duration_since(Instant::now());
        let seconds = remaining
            .as_secs()
            .try_into()
            .map_err(|_| Error::Deadline)?;
        let timeout = libc::timespec {
            tv_sec: seconds,
            tv_nsec: remaining.subsec_nanos().into(),
        };
        let count = match queue.kevent(pending, &mut events, Some(timeout)) {
            Err(Errno::EINTR) => continue,
            result => result?,
        };
        pending = &[];
        if let Some(event) = events[..count].first() {
            return decode(event, child);
        }
        if Instant::now() >= deadline {
            return Ok(Outcome::TimedOut);
        }
    }
}

/// Preserve registration failures and reap only the exact child whose exit was observed.
fn decode(event: &KEvent, child: &mut Child) -> Result<Outcome, Error> {
    if event.flags().contains(EvFlags::EV_ERROR) {
        let code = i32::try_from(event.data()).map_err(|_| Error::Event)?;
        // A child can exit between try_wait and registration. Reap that exact
        // child before interpreting ESRCH as an event failure.
        if code == libc::ESRCH
            && let Some(status) = child.try_wait()?
        {
            return Ok(Outcome::Exited(status));
        }
        return Err(Error::Kernel(Errno::from_raw(code)));
    }
    match event.filter()? {
        EventFilter::EVFILT_READ => Ok(Outcome::Cancelled),
        EventFilter::EVFILT_PROC => {
            let status = child.try_wait()?.ok_or(Error::MissingExit)?;
            Ok(Outcome::Exited(status))
        }
        _ => Err(Error::Event),
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::process::Command;
    use std::thread;

    use super::*;

    /// Always stop the bounded test child, including after an assertion fails.
    struct Process(Child);

    impl Drop for Process {
        fn drop(&mut self) {
            let _ = self.0.kill();
            let _ = self.0.wait();
        }
    }

    /// Use a real finite process rather than a mocked kernel event.
    fn sleeper() -> Process {
        Process(Command::new("/bin/sleep").arg("5").spawn().unwrap())
    }

    #[test]
    fn child_exit_preserves_its_status() {
        let mut child = Process(
            Command::new("/bin/sh")
                .args(["-c", "exit 7"])
                .spawn()
                .unwrap(),
        );
        let (_caller, cancel) = UnixStream::pair().unwrap();
        let Outcome::Exited(status) =
            wait(&mut child.0, Some(&cancel), Duration::from_secs(2)).unwrap()
        else {
            panic!("expected exit")
        };
        assert_eq!(status.code(), Some(7));
    }

    #[test]
    fn caller_disconnect_wakes_the_supervisor() {
        let mut child = sleeper();
        let (caller, cancel) = UnixStream::pair().unwrap();
        let sender = thread::spawn(move || {
            thread::sleep(Duration::from_millis(20));
            drop(caller);
        });
        assert!(matches!(
            wait(&mut child.0, Some(&cancel), Duration::from_secs(2)).unwrap(),
            Outcome::Cancelled
        ));
        sender.join().unwrap();
        assert!(child.0.try_wait().unwrap().is_none());
    }

    #[test]
    fn cancellation_data_wakes_the_supervisor() {
        let mut child = sleeper();
        let (mut caller, cancel) = UnixStream::pair().unwrap();
        caller.write_all(&[1]).unwrap();
        assert!(matches!(
            wait(&mut child.0, Some(&cancel), Duration::from_secs(2)).unwrap(),
            Outcome::Cancelled
        ));
    }

    #[test]
    fn deadline_wakes_the_supervisor_without_io() {
        let mut child = sleeper();
        let (_caller, cancel) = UnixStream::pair().unwrap();
        assert!(matches!(
            wait(&mut child.0, Some(&cancel), Duration::from_millis(20)).unwrap(),
            Outcome::TimedOut
        ));
        assert!(child.0.try_wait().unwrap().is_none());
    }
}
