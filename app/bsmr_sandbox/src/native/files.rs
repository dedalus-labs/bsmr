//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Transfer opened action files to the native worker without privileged path lookup.

use std::fs::File;
use std::io;
use std::io::IoSlice;
use std::io::IoSliceMut;
use std::os::fd::AsFd;
use std::os::fd::AsRawFd;
use std::os::fd::FromRawFd;
use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;

use nix::errno::Errno;
use nix::fcntl::FcntlArg;
use nix::fcntl::FdFlag;
use nix::fcntl::OFlag;
use nix::fcntl::fcntl;
use nix::poll::PollFd;
use nix::poll::PollFlags;
use nix::poll::poll;
use nix::sys::socket::ControlMessage;
use nix::sys::socket::ControlMessageOwned;
use nix::sys::socket::MsgFlags;
use nix::sys::socket::recvmsg;
use nix::sys::socket::sendmsg;
use nix::unistd::Uid;
use nix::unistd::getpeereid;
use thiserror::Error;

/// Three file capabilities. The receiver owns its descriptors independently of the sender.
pub struct Files {
    /// Read-only action description. Parsing and size limits belong to the worker.
    pub action: File,
    /// Read-only input archive. The worker snapshots and verifies its bytes before use.
    pub input: File,
    /// Read/write result archive. Never publish it before acknowledged cleanup.
    pub output: File,
}

/// Transport failures occur before action parsing or execution.
#[derive(Debug, Error)]
pub enum Error {
    #[error("native client UID {actual} does not match configured UID {expected}")]
    Peer { expected: u32, actual: u32 },
    #[error("native request must transfer exactly three files with protocol byte 1")]
    Frame,
    #[error("native client did not send its descriptor packet within five seconds")]
    Deadline,
    #[error("native transport file {0} has the wrong kind or access mode")]
    File(usize),
    #[error("native transport syscall failed: {0}")]
    Kernel(#[from] Errno),
    #[error("native transport I/O failed: {0}")]
    Io(#[from] io::Error),
}

impl Files {
    /// Send one bounded descriptor packet. Keep the connection open through completion.
    pub fn send(&self, stream: &impl AsFd) -> io::Result<()> {
        let descriptors = [
            self.action.as_raw_fd(),
            self.input.as_raw_fd(),
            self.output.as_raw_fd(),
        ];
        let count = sendmsg::<()>(
            stream.as_fd().as_raw_fd(),
            &[IoSlice::new(&[1])],
            &[ControlMessage::ScmRights(&descriptors)],
            MsgFlags::empty(),
            None,
        )
        .map_err(io::Error::from)?;
        if count != 1 {
            return Err(io::ErrorKind::WriteZero.into());
        }
        Ok(())
    }

    /// Authenticate the kernel peer before accepting any file capabilities.
    ///
    /// `allowed` comes from administrator configuration. The service must receive
    /// before starting threads or children: Darwin lacks atomic CLOEXEC receipt.
    /// Each received descriptor is owned immediately and closes on any refusal.
    pub fn receive(stream: &UnixStream, allowed: Uid) -> Result<Self, Error> {
        let (peer, _) = getpeereid(stream)?;
        if peer != allowed {
            return Err(Error::Peer {
                expected: allowed.as_raw(),
                actual: peer.as_raw(),
            });
        }
        let mut readiness = [PollFd::new(stream.as_fd(), PollFlags::POLLIN)];
        if poll(&mut readiness, 5_000_u16)? == 0 {
            return Err(Error::Deadline);
        }
        let mut byte = [0];
        let mut iov = [IoSliceMut::new(&mut byte)];
        let mut control = nix::cmsg_space!([i32; 3]);
        let message = recvmsg::<()>(
            stream.as_raw_fd(),
            &mut iov,
            Some(&mut control),
            MsgFlags::MSG_DONTWAIT,
        )?;
        let count = message.bytes;
        let truncated = message.flags.contains(MsgFlags::MSG_CTRUNC);
        let mut files = Vec::new();
        for control in message.cmsgs()? {
            if let ControlMessageOwned::ScmRights(descriptors) = control {
                for descriptor in descriptors {
                    // SAFETY: SCM_RIGHTS gives this process a new, uniquely owned descriptor.
                    let owned = unsafe { OwnedFd::from_raw_fd(descriptor) };
                    files.push(owned);
                }
            }
        }
        if count != 1 || byte != [1] || truncated || files.len() != 3 {
            return Err(Error::Frame);
        }
        for file in &files {
            fcntl(file, FcntlArg::F_SETFD(FdFlag::FD_CLOEXEC))?;
        }
        let [action, input, output]: [_; 3] = files.try_into().map_err(|_| Error::Frame)?;
        let files = Self {
            action: action.into(),
            input: input.into(),
            output: output.into(),
        };
        files.validate()?;
        Ok(files)
    }

    /// Reject device capabilities and modes that could mutate input bytes.
    fn validate(&self) -> Result<(), Error> {
        for (index, file) in [&self.action, &self.input, &self.output]
            .into_iter()
            .enumerate()
        {
            let access = fcntl(file, FcntlArg::F_GETFL)? & OFlag::O_ACCMODE.bits();
            let expected = if index == 2 {
                OFlag::O_RDWR
            } else {
                OFlag::O_RDONLY
            };
            if !file.metadata()?.is_file() || access != expected.bits() {
                return Err(Error::File(index));
            }
        }
        Ok(())
    }
}
