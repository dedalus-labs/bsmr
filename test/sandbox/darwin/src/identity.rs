//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Hold one unused Darwin identity until all of its processes have stopped.

use std::fs::File;
use std::fs::OpenOptions;
use std::io;
use std::os::unix::fs::MetadataExt;
use std::os::unix::fs::OpenOptionsExt;
use std::process::Command;
use std::process::Stdio;
use std::thread;
use std::time::Duration;
use std::time::Instant;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use anyhow::ensure;
use nix::errno::Errno;
use nix::fcntl::Flock;
use nix::fcntl::FlockArg;
use nix::unistd::Gid;
use nix::unistd::Group;
use nix::unistd::Pid;
use nix::unistd::Uid;
use nix::unistd::User;
use nix::unistd::setgid;
use nix::unistd::setuid;

/// The probe refuses this identity if any account, group, or process already uses it.
pub(crate) const ID: u32 = 60_000;
/// Serialize every invocation of this probe. Keep the inode after unlocking.
const LOCK: &str = "/var/run/bsmr-darwin-60000.lock";
/// libproc.h identifies real-UID membership with this selector.
const PROC_RUID_ONLY: u32 = 5;
/// syscall.h assigns 37 to Darwin's kill implementation.
const SYS_KILL: libc::c_int = 37;

/// Hold the exclusive identity until its subprocesses are drained.
pub(crate) struct Lease {
    /// Owns the root-only file lock. It cannot cross exec.
    #[expect(
        dead_code,
        reason = "the descriptor holds the identity lease until Drop finishes"
    )]
    lock: Flock<File>,
}

impl Lease {
    /// Reject occupied identities before creating any process with this UID.
    pub(crate) fn acquire() -> Result<Self> {
        ensure!(
            Uid::current().is_root() && Uid::effective().is_root(),
            "root required"
        );
        ensure!(
            User::from_uid(Uid::from_raw(ID))?.is_none(),
            "UID {ID} belongs to an account"
        );
        ensure!(
            Group::from_gid(Gid::from_raw(ID))?.is_none(),
            "GID {ID} belongs to a group"
        );
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
            .open(LOCK)?;
        let metadata = file.metadata()?;
        ensure!(
            metadata.is_file() && metadata.uid() == 0 && metadata.mode() & 0o777 == 0o600,
            "lease file must be a root-owned regular file with mode 0600"
        );
        let lock = Flock::lock(file, FlockArg::LockExclusiveNonblock)
            .map_err(|(_, error)| error)
            .context("identity lease is busy")?;
        ensure!(member(ID)?.is_none(), "UID {ID} already has processes");
        Ok(Self { lock })
    }

    /// Broadcast only after the helper drops all credentials to the reserved UID.
    pub(crate) fn drain(&self) -> Result<()> {
        let deadline = Instant::now() + Duration::from_secs(4);
        while member(ID)?.is_some() {
            ensure!(
                Instant::now() < deadline,
                "UID {ID} did not drain before its deadline"
            );
            let status = Command::new(std::env::current_exe()?)
                .arg("kill")
                .env_clear()
                .stdin(Stdio::null())
                .status()?;
            ensure!(
                status.success(),
                "credential-scoped kill helper failed: {status}"
            );
            // Darwin has no supported UID-empty notification. This bounded probe samples
            // membership after the broadcast, including zombies waiting for launchd to reap.
            thread::sleep(Duration::from_millis(5));
        }
        Ok(())
    }
}

impl Drop for Lease {
    /// Keep failures visible while attempting cleanup on every early return.
    fn drop(&mut self) {
        if let Err(error) = self.drain() {
            eprintln!("cleanup incomplete, UID {ID} must remain reserved: {error:#}");
        }
    }
}

/// Ask the kernel for one member. A failed query is never interpreted as an empty UID.
pub(crate) fn member(uid: u32) -> Result<Option<Pid>> {
    let mut pid: libc::pid_t = 0;
    let size = libc::c_int::try_from(size_of::<libc::pid_t>())?;
    // SAFETY: the writable buffer has exactly the supplied length. errno is thread-local.
    let (bytes, error) = unsafe {
        *libc::__error() = 0;
        let bytes = libc::proc_listpids(
            PROC_RUID_ONLY,
            uid,
            (&mut pid as *mut libc::pid_t).cast(),
            size,
        );
        (bytes, *libc::__error())
    };
    ensure!(
        bytes >= 0 && error == 0,
        "proc_listpids({uid}) failed: {}",
        io::Error::from_raw_os_error(error)
    );
    match bytes {
        0 => Ok(None),
        value if value == size && pid > 0 => Ok(Some(Pid::from_raw(pid))),
        _ => bail!("unexpected process membership result: bytes={bytes} pid={pid}"),
    }
}

/// Permanently relinquish root before any broadcast or workload operation.
pub(crate) fn enter() -> Result<()> {
    ensure!(
        Uid::current().is_root() && Uid::effective().is_root(),
        "credential drop requires root"
    );
    // SAFETY: an empty group vector permits a null pointer. Darwin uses c_int for its length.
    Errno::result(unsafe { libc::setgroups(0, std::ptr::null()) })?;
    setgid(Gid::from_raw(ID))?;
    setuid(Uid::from_raw(ID))?;
    ensure!(
        Uid::current().as_raw() == ID && Uid::effective().as_raw() == ID,
        "UID drop failed"
    );
    ensure!(
        Gid::current().as_raw() == ID && Gid::effective().as_raw() == ID,
        "GID drop failed"
    );
    ensure!(
        setuid(Uid::from_raw(0)) == Err(Errno::EPERM),
        "saved root credentials survived"
    );
    ensure!(
        setgid(Gid::from_raw(0)) == Err(Errno::EPERM),
        "saved root group survived"
    );
    Ok(())
}

/// Use Darwin's non-POSIX broadcast flag so the trusted helper can report completion.
#[allow(deprecated)] // Darwin exposes the non-POSIX flag only through its syscall entrypoint.
pub(crate) fn broadcast() -> Result<()> {
    enter()?;
    // SAFETY: fixed ABI arguments. enter() proves this cannot broadcast with root credentials.
    let result =
        unsafe { libc::syscall(SYS_KILL, -1 as libc::pid_t, libc::SIGKILL, 0 as libc::c_int) };
    ensure!(
        result == 0 || Errno::last() == Errno::ESRCH,
        "kill broadcast failed: {}",
        Errno::last()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invariant_current_identity_is_not_reported_empty() -> Result<()> {
        ensure!(member(Uid::current().as_raw())?.is_some());
        Ok(())
    }

    #[test]
    fn invariant_unprivileged_caller_cannot_acquire_identity() -> Result<()> {
        ensure!(!Uid::effective().is_root(), "run this control without sudo");
        ensure!(Lease::acquire().is_err());
        ensure!(enter().is_err());
        Ok(())
    }
}
