//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Confine the trusted launcher before it executes an action's code.

use std::ffi::CStr;
use std::io;
use std::path::Path;

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
