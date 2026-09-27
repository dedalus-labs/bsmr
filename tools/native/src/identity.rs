//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Hold a reserved identity until its complete process population has stopped.

use std::fs::File;
use std::fs::OpenOptions;
use std::io;
use std::os::fd::{AsFd, BorrowedFd};
use std::os::unix::fs::MetadataExt;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::path::PathBuf;
use std::thread;
use std::time::Duration;
use std::time::Instant;

use nix::errno::Errno;
use nix::fcntl::Flock;
use nix::fcntl::FlockArg;
use nix::sys::wait::WaitStatus;
use nix::sys::wait::waitpid;
use nix::unistd::ForkResult;
use nix::unistd::Gid;
use nix::unistd::Group;
use nix::unistd::Uid;
use nix::unistd::User;
use thiserror::Error;

/// Refusals retain the identity and failing operating-system operation.
#[derive(Debug, Error)]
pub enum Error {
    #[error("native identity ownership requires real and effective root")]
    Root,
    #[error("UID/GID {0} belongs to an account, group, or existing process")]
    Occupied(u32),
    #[error("identity lease path lacks protected root ownership: {0:?}")]
    Lease(PathBuf),
    #[error("identity lease is already held")]
    Busy,
    #[error("UID {0} still has processes after the cleanup deadline")]
    Deadline(u32),
    #[error("credential-scoped cleanup failed for UID {0}")]
    Cleanup(u32),
    #[error("kernel process-membership response is invalid: {0} bytes")]
    Membership(i32),
    #[error("identity filesystem operation failed: {0}")]
    Io(#[from] io::Error),
    #[error("identity kernel operation failed: {0}")]
    Kernel(#[from] Errno),
}

/// An exclusive, unused UID/GID selected by administrator configuration.
///
/// Keep this owner through cleanup and output import. Acquisition rejects any
/// surviving process, including one left by a crashed supervisor. A cleanup
/// error never makes that identity available for another action.
pub struct Identity {
    /// Kernel lock on an administrator-created file. Never remove its inode.
    _lease: Flock<File>,
    /// No account, group, or process owned this ID when the lease was acquired.
    id: u32,
}

impl Identity {
    /// Lease an administrator-selected identity using an existing protected file.
    ///
    /// The caller supplies trusted configuration, never fields from an action.
    /// Every ancestor of `path` must already be root-owned and unwritable by others.
    pub fn acquire(path: &Path, id: u32) -> Result<Self, Error> {
        root()?;
        if id == 0
            || User::from_uid(Uid::from_raw(id))?.is_some()
            || Group::from_gid(Gid::from_raw(id))?.is_some()
        {
            return Err(Error::Occupied(id));
        }
        for ancestor in path.ancestors().skip(1) {
            let metadata = std::fs::symlink_metadata(ancestor)?;
            if !metadata.is_dir() || metadata.uid() != 0 || metadata.mode() & 0o022 != 0 {
                return Err(Error::Lease(ancestor.to_owned()));
            }
        }
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(path)?;
        let metadata = file.metadata()?;
        if !metadata.is_file() || metadata.uid() != 0 || metadata.mode() & 0o777 != 0o600 {
            return Err(Error::Lease(path.to_owned()));
        }
        let lease = Flock::lock(file, FlockArg::LockExclusiveNonblock).map_err(|(_, error)| {
            if error == Errno::EWOULDBLOCK {
                Error::Busy
            } else {
                Error::Kernel(error)
            }
        })?;
        if occupied(id)? {
            return Err(Error::Occupied(id));
        }
        Ok(Self { _lease: lease, id })
    }

    /// Return the reserved UID/GID for the trusted credential-dropping child.
    #[must_use]
    pub fn id(&self) -> u32 {
        self.id
    }

    /// Keep the same kernel lease alive across the trusted launcher's privilege transition.
    pub(crate) fn descriptor(&self) -> BorrowedFd<'_> {
        self._lease.as_fd()
    }

    /// Stop all detached descendants and require an empty kernel process set.
    pub fn drain(&self) -> Result<(), Error> {
        root()?;
        let deadline = Instant::now() + Duration::from_secs(4);
        while occupied(self.id)? {
            if Instant::now() >= deadline {
                return Err(Error::Deadline(self.id));
            }
            self.broadcast()?;
            // Darwin has no UID-empty event. Observe reaping only after signalling.
            thread::sleep(Duration::from_millis(5));
        }
        Ok(())
    }

    /// Signal as the build UID, so the kernel cannot target another user's processes.
    fn broadcast(&self) -> Result<(), Error> {
        // SAFETY: the child calls only async-signal-safe syscalls and _exit. It
        // never allocates, unwinds, drops inherited Rust values, or returns.
        match unsafe { nix::unistd::fork()? } {
            ForkResult::Child => unsafe {
                let dropped = libc::setgroups(0, std::ptr::null()) == 0
                    && libc::setgid(self.id) == 0
                    && libc::setuid(self.id) == 0;
                #[allow(deprecated)] // Darwin's flag excludes the signalling process itself.
                let killed = dropped
                    && (libc::syscall(37, -1 as libc::pid_t, libc::SIGKILL, 0 as libc::c_int) == 0
                        || *libc::__error() == libc::ESRCH);
                libc::_exit(if killed { 0 } else { 1 });
            },
            ForkResult::Parent { child } => loop {
                match waitpid(child, None) {
                    Ok(WaitStatus::Exited(_, 0)) => return Ok(()),
                    Err(Errno::EINTR) => continue,
                    Err(error) => return Err(error.into()),
                    Ok(_) => return Err(Error::Cleanup(self.id)),
                }
            },
        }
    }
}

impl Drop for Identity {
    /// Attempt cleanup on early returns. A surviving UID stays unavailable on next acquisition.
    fn drop(&mut self) {
        if let Err(error) = self.drain() {
            eprintln!("native identity cleanup incomplete: {error}");
        }
    }
}

/// Reject unprivileged and partially elevated callers before any identity operation.
fn root() -> Result<(), Error> {
    if Uid::current().is_root() && Uid::effective().is_root() {
        Ok(())
    } else {
        Err(Error::Root)
    }
}

/// Ask the kernel for one member. Failed queries never mean the identity is empty.
pub fn occupied(id: u32) -> Result<bool, Error> {
    let mut pid: libc::pid_t = 0;
    let size = size_of::<libc::pid_t>() as libc::c_int;
    // SAFETY: the writable PID buffer has the supplied size. errno is thread-local.
    let (bytes, error) = unsafe {
        *libc::__error() = 0;
        let bytes = libc::proc_listpids(5, id, (&mut pid as *mut libc::pid_t).cast(), size);
        (bytes, *libc::__error())
    };
    if error != 0 {
        return Err(io::Error::from_raw_os_error(error).into());
    }
    match bytes {
        0 => Ok(false),
        value if value == size && pid > 0 => Ok(true),
        _ => Err(Error::Membership(bytes)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_identity_is_not_empty() {
        assert!(occupied(Uid::current().as_raw()).unwrap());
    }

    #[test]
    fn unprivileged_caller_cannot_claim_an_identity() {
        assert!(!Uid::current().is_root());
        assert!(matches!(
            Identity::acquire(Path::new("/unused"), 60_000),
            Err(Error::Root)
        ));
    }
}
