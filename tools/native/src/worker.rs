//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Serve authenticated native jobs through launchd's persistent listening socket.

use std::ffi::OsStr;
use std::fs;
use std::io::{self, Read, Write};
use std::os::fd::FromRawFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use bsmr_sandbox::{LauncherResponse, LauncherStatus, PROTOCOL_VERSION};
use nix::unistd::{Uid, User};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::channel::Files;
use crate::identity::Identity;
use crate::job::Job;
use crate::run::Outcome;
use crate::runtime::Runtime;

unsafe extern "C" {
    fn launch_activate_socket(
        name: *const libc::c_char,
        descriptors: *mut *mut libc::c_int,
        count: *mut libc::size_t,
    ) -> libc::c_int;
}

/// Administrator-owned configuration. Requests cannot choose paths or identities.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// OS account authorized to submit builds.
    pub client: u32,
    /// Unused UID/GID reserved for this worker's serial action lane.
    pub identity: u32,
    /// Root-owned immutable runtime seed.
    pub seed: PathBuf,
    /// Private root-owned directory containing the identity lease and job roots.
    pub state: PathBuf,
    /// Socket path configured in the launchd service.
    pub socket: PathBuf,
}

/// Published before admission so the executor can bind its cache key to actual bytes.
#[derive(Deserialize, Serialize)]
pub struct Info {
    /// Protocol understood by the installed worker.
    pub protocol: u32,
    /// Digest of the retained runtime snapshot and trusted launcher.
    pub environment: String,
    /// Maximum simultaneous executions in this worker.
    pub slots: usize,
}

/// Startup failures do not admit requests or select another executor.
#[derive(Debug, Error)]
pub enum Error {
    #[error(
        "native worker requires root and an existing client account distinct from its action identity"
    )]
    Identity,
    #[error("native worker configuration must be a regular file of at most 64 KiB")]
    Config,
    #[error("launchd did not supply exactly one native listener: {0}")]
    Listener(i32),
    #[error(
        "native listener {actual:?} does not match configured socket {expected:?}; socket_file={socket_file}"
    )]
    Socket {
        expected: PathBuf,
        actual: Option<PathBuf>,
        socket_file: bool,
    },
    #[error("native worker I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("native worker encoding failed: {0}")]
    Encoding(#[from] serde_json::Error),
    #[error("native worker runtime failed: {0}")]
    Runtime(#[from] crate::runtime::Error),
    #[error("native worker account lookup failed: {0}")]
    Account(#[from] nix::errno::Errno),
}

/// Process one job at a time. Runtime teardown joins cleanup threads before the next receipt.
pub fn serve(path: &Path) -> Result<(), Error> {
    if !Uid::current().is_root() || !Uid::effective().is_root() {
        return Err(Error::Identity);
    }
    crate::runtime::protected(path)?;
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_file() || metadata.len() > 64 * 1024 {
        return Err(Error::Config);
    }
    let mut bytes = Vec::new();
    fs::File::open(path)?
        .take(64 * 1024 + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() > 64 * 1024 {
        return Err(Error::Config);
    }
    let config: Config = serde_json::from_slice(&bytes)?;
    if config.client == config.identity || User::from_uid(Uid::from_raw(config.client))?.is_none() {
        return Err(Error::Identity);
    }
    let listener = listener()?;
    let actual = listener.local_addr()?.as_pathname().map(socket_name);
    let socket_file = fs::symlink_metadata(&config.socket)?
        .file_type()
        .is_socket();
    if actual.as_ref() != Some(&config.socket) || !socket_file {
        return Err(Error::Socket {
            expected: config.socket,
            actual,
            socket_file,
        });
    }
    let public = config.socket.parent().ok_or(Error::Config)?;
    crate::runtime::protected(public)?;
    fs::set_permissions(&config.socket, fs::Permissions::from_mode(0o600))?;
    std::os::unix::fs::chown(&config.socket, Some(config.client), None)?;
    let launcher = std::env::current_exe()?;
    let image = Runtime::capture(&config.seed, &config.state, &launcher)?;
    let info = Info {
        protocol: PROTOCOL_VERSION,
        environment: image.digest().to_owned(),
        slots: 1,
    };
    let mut temporary = tempfile::NamedTempFile::new_in(public)?;
    serde_json::to_writer(&mut temporary, &info)?;
    temporary
        .as_file()
        .set_permissions(fs::Permissions::from_mode(0o644))?;
    temporary
        .persist(config.socket.with_extension("json"))
        .map_err(|error| error.error)?;
    for stream in listener.incoming() {
        let mut stream = stream?;
        let response = match execute(&config, &image, &launcher, &stream) {
            Ok(response) => response,
            Err(error) => LauncherResponse {
                protocol: PROTOCOL_VERSION,
                status: LauncherStatus::Failed,
                cleanup_complete: false,
                error: Some(error.to_string()),
            },
        };
        if let Err(error) = stream
            .set_nonblocking(false)
            .and_then(|()| stream.set_write_timeout(Some(Duration::from_secs(5))))
            .and_then(|()| serde_json::to_writer(&mut stream, &response).map_err(io::Error::other))
            .and_then(|()| stream.write_all(b"\n"))
        {
            eprintln!("native response could not be delivered: {error}");
        }
    }
    Ok(())
}

/// Authenticate and receive before creating any threads or action processes.
fn execute(
    config: &Config,
    image: &Runtime,
    launcher: &Path,
    stream: &UnixStream,
) -> Result<LauncherResponse, Box<dyn std::error::Error>> {
    let files = Files::receive(stream, Uid::from_raw(config.client))?;
    let identity = Identity::acquire(&config.state.join("identity"), config.identity)?;
    let job = Job::prepare(identity, image.instantiate()?, files, image.digest())?;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(async {
        let control = stream.try_clone()?;
        control.set_nonblocking(true)?;
        let control = tokio::net::UnixStream::from_std(control)?;
        let mut completed = job.run(launcher.to_owned(), control).await?;
        crate::output::write(&mut completed)?;
        let status = match completed.outcome() {
            Outcome::Exited(_) => LauncherStatus::Completed,
            Outcome::Cancelled => LauncherStatus::Cancelled,
            Outcome::TimedOut => LauncherStatus::TimedOut,
        };
        Ok(LauncherResponse {
            protocol: PROTOCOL_VERSION,
            status,
            cleanup_complete: true,
            error: None,
        })
    })
}

/// Adopt launchd's listener instead of replacing a possibly live Unix socket on startup.
fn listener() -> Result<UnixListener, Error> {
    let mut descriptors = std::ptr::null_mut();
    let mut count = 0;
    // SAFETY: launchd allocates the descriptor array for these valid out-pointers.
    let error =
        unsafe { launch_activate_socket(c"Listener".as_ptr(), &mut descriptors, &mut count) };
    if error != 0 {
        return Err(Error::Listener(error));
    }
    let mut owned = Vec::new();
    if !descriptors.is_null() {
        // SAFETY: launchd returned `count` valid, newly owned descriptors.
        for descriptor in unsafe { std::slice::from_raw_parts(descriptors, count) } {
            owned.push(unsafe { UnixListener::from_raw_fd(*descriptor) });
        }
        unsafe { libc::free(descriptors.cast()) };
    }
    if owned.len() != 1 {
        return Err(Error::Listener(0));
    }
    let listener = owned.pop().expect("one verified listener");
    nix::fcntl::fcntl(
        &listener,
        nix::fcntl::FcntlArg::F_SETFD(nix::fcntl::FdFlag::FD_CLOEXEC),
    )?;
    Ok(listener)
}

/// launchd binds a full sockaddr_un. Rust preserves its trailing NUL padding.
fn socket_name(path: &Path) -> PathBuf {
    let bytes = path.as_os_str().as_bytes();
    let name = bytes
        .split(|byte| *byte == 0)
        .next()
        .expect("one C string prefix");
    PathBuf::from(OsStr::from_bytes(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kernel_padding_does_not_change_the_bound_socket_name() {
        let expected = Path::new("/private/var/db/bsmr/control.sock");
        assert_eq!(socket_name(expected), expected);
        let padded = Path::new(OsStr::from_bytes(
            b"/private/var/db/bsmr/control.sock\0\0\0",
        ));
        assert_eq!(socket_name(padded), expected);
        assert_ne!(
            socket_name(Path::new("/private/var/db/bsmr/control.sock.other\0")),
            expected
        );
    }
}
