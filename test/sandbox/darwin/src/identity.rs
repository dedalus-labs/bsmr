//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Drop the bounded qualification workload into its leased identity.

use std::fs::OpenOptions;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use anyhow::Result;
use anyhow::ensure;
use bsmr_native::identity::Identity;
use nix::errno::Errno;
use nix::unistd::Gid;
use nix::unistd::Uid;
use nix::unistd::setgid;
use nix::unistd::setuid;

pub(crate) const ID: u32 = 60_000;
const LOCK: &str = "/private/var/root/bsmr-darwin-60000.lock";

/// Create the disposable runner's administrator-owned lease, then use the real owner.
pub(crate) fn acquire() -> Result<Identity> {
    ensure!(Uid::current().is_root() && Uid::effective().is_root());
    OpenOptions::new()
        .write(true)
        .create(true)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC)
        .open(LOCK)?;
    Ok(Identity::acquire(Path::new(LOCK), ID)?)
}

/// Permanently relinquish all root credentials before the finite test workload.
pub(crate) fn enter() -> Result<()> {
    ensure!(Uid::current().is_root() && Uid::effective().is_root());
    // SAFETY: an empty group vector permits a null pointer. Darwin uses c_int for its length.
    Errno::result(unsafe { libc::setgroups(0, std::ptr::null()) })?;
    setgid(Gid::from_raw(ID))?;
    setuid(Uid::from_raw(ID))?;
    ensure!(Uid::current().as_raw() == ID && Uid::effective().as_raw() == ID);
    ensure!(Gid::current().as_raw() == ID && Gid::effective().as_raw() == ID);
    ensure!(setuid(Uid::from_raw(0)) == Err(Errno::EPERM));
    ensure!(setgid(Gid::from_raw(0)) == Err(Errno::EPERM));
    Ok(())
}
