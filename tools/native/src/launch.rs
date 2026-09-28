//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Confine the trusted launcher before it executes an action's code.

use std::ffi::CStr;
use std::fs::File;
use std::io;
use std::io::Read;
use std::os::fd::FromRawFd;
use std::os::unix::fs::MetadataExt;
use std::os::unix::process::CommandExt;
use std::path::Path;
use std::process::Command;

use bsmr_sandbox::{GuestAction, MAX_ACTION_BYTES};
use nix::errno::Errno;
use nix::unistd::Gid;
use nix::unistd::Uid;
use nix::unistd::chdir;
use nix::unistd::chroot;
use nix::unistd::setgid;
use nix::unistd::setuid;
use thiserror::Error;

unsafe extern "C" {
    fn sandbox_init(
        profile: *const libc::c_char,
        flags: u64,
        error: *mut *mut libc::c_char,
    ) -> libc::c_int;
    fn sandbox_free_error(error: *mut libc::c_char);
}

/// A failed transition must terminate the launcher without executing the action.
#[derive(Debug, Error)]
pub enum Error {
    #[error("native launch requires root and a non-root reserved identity")]
    Identity,
    #[error("native confinement syscall failed: {0}")]
    Kernel(#[from] Errno),
    #[error("native sandbox policy was refused: {0}")]
    Policy(String),
    #[error("native launch I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("native launch action is invalid: {0}")]
    Action(#[from] serde_json::Error),
    #[error("native launcher did not inherit a protected identity lease")]
    Lease,
}

/// Read the supervisor's private action, confine this child, then replace it with the payload.
///
/// The supervisor wrote this bounded file from a validated request. Action
/// environment variables are applied only after root credentials are gone.
pub fn action(root: &Path, uid: u32, descriptor: i32) -> Result<std::convert::Infallible, Error> {
    let _lease = inherited(descriptor)?;
    let file = File::open(root.join(".bsmr/action.json"))?;
    let action: GuestAction = serde_json::from_reader(file.take(MAX_ACTION_BYTES))?;
    enter(root, uid)?;
    let mut command = Command::new(&action.arguments[0]);
    command
        .args(&action.arguments[1..])
        .current_dir(Path::new("/workspace").join(action.working_directory))
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", "/tmp")
        .env("TMPDIR", "/tmp")
        .env("BSMR_SCRATCH_PATH", "/tmp")
        .envs(action.environment);
    Err(command.exec().into())
}

/// Adopt the supervisor's duplicate, closing it automatically at payload exec.
fn inherited(descriptor: i32) -> Result<File, Error> {
    if !Uid::current().is_root() || !Uid::effective().is_root() || descriptor < 3 {
        return Err(Error::Identity);
    }
    // SAFETY: this runs in the single-threaded launcher before any action code.
    // A successful fcntl proves the inherited descriptor exists and remains open.
    Errno::result(unsafe { libc::fcntl(descriptor, libc::F_SETFD, libc::FD_CLOEXEC) })?;
    // SAFETY: the parent deliberately transferred this descriptor across exec.
    let lease = unsafe { File::from_raw_fd(descriptor) };
    let metadata = lease.metadata()?;
    if !metadata.is_file() || metadata.uid() != 0 || metadata.mode() & 0o777 != 0o600 {
        return Err(Error::Lease);
    }
    Ok(lease)
}

/// Enter an administrator-owned private root, then permanently drop privileges.
///
/// Call only in a dedicated, single-threaded launcher, never in `pre_exec`.
/// The supervisor must hold this UID's `Identity` lease. It owns and validates
/// `root`, and closes every inherited descriptor except declared action I/O.
/// The caller must exit on error. Successful callers may execute the payload.
pub fn enter(root: &Path, uid: u32) -> Result<(), Error> {
    if !Uid::current().is_root() || !Uid::effective().is_root() || uid == 0 {
        return Err(Error::Identity);
    }
    chroot(root)?;
    chdir("/")?;
    // SAFETY: an empty group vector permits a null pointer. No other threads exist.
    Errno::result(unsafe { libc::setgroups(0, std::ptr::null()) })?;
    setgid(Gid::from_raw(uid))?;
    setuid(Uid::from_raw(uid))?;
    policy()
}

/// Deny host services and networking after the filesystem and credentials change.
fn policy() -> Result<(), Error> {
    let profile = c"(version 1)(deny default)(allow file* process* sysctl-read system-socket)(allow signal (target same-sandbox))";
    let mut error = std::ptr::null_mut();
    // SAFETY: the profile is static and terminated. The out-pointer remains valid
    // until this synchronous call returns. Apple's API allocates any error string.
    let result = unsafe { sandbox_init(profile.as_ptr(), 0, &mut error) };
    let message = if error.is_null() {
        io::Error::last_os_error().to_string()
    } else {
        // SAFETY: sandbox_init returned a terminated string owned by its matching free API.
        let message = unsafe { CStr::from_ptr(error) }
            .to_string_lossy()
            .into_owned();
        unsafe { sandbox_free_error(error) };
        message
    };
    if result == 0 {
        Ok(())
    } else {
        Err(Error::Policy(message))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn root_is_never_an_action_identity() {
        assert!(matches!(enter(Path::new("/"), 0), Err(Error::Identity)));
    }
}
