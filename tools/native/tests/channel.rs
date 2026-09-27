//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Exercise actual kernel peer credentials and descriptor ownership.

#![cfg(target_os = "macos")]

use std::fs::File;
use std::io::{Read, Write};
use std::os::unix::net::UnixStream;

use bsmr_sandbox::native::files::{Error, Files};
use nix::fcntl::{FcntlArg, FdFlag, fcntl};
use nix::unistd::Uid;

/// Use real regular files with the exact required access modes.
fn files() -> Files {
    let mut input = tempfile::NamedTempFile::new().unwrap();
    input.write_all(b"input").unwrap();
    Files {
        action: File::open(input.path()).unwrap(),
        input: File::open(input.path()).unwrap(),
        output: tempfile::tempfile().unwrap(),
    }
}

#[test]
fn received_files_survive_sender_close_without_becoming_inheritable() {
    let (client, server) = UnixStream::pair().unwrap();
    files().send(&client).unwrap();
    drop(client);
    let mut files = Files::receive(&server, Uid::current()).unwrap();
    let mut bytes = String::new();
    files.input.read_to_string(&mut bytes).unwrap();
    assert_eq!(bytes, "input");
    files.output.write_all(b"result").unwrap();
    for file in [&files.action, &files.input, &files.output] {
        assert_eq!(
            fcntl(file, FcntlArg::F_GETFD).unwrap(),
            FdFlag::FD_CLOEXEC.bits()
        );
    }
}

#[test]
fn peer_identity_does_not_come_from_the_request() {
    let (_client, server) = UnixStream::pair().unwrap();
    let different = Uid::from_raw(Uid::current().as_raw() ^ 1);
    assert!(matches!(
        Files::receive(&server, different),
        Err(Error::Peer { .. })
    ));
}

#[test]
fn protocol_requires_file_capabilities() {
    let (mut client, server) = UnixStream::pair().unwrap();
    client.write_all(&[1]).unwrap();
    assert!(matches!(
        Files::receive(&server, Uid::current()),
        Err(Error::Frame)
    ));
}

#[test]
fn disconnected_client_cannot_leave_receipt_waiting() {
    let (client, server) = UnixStream::pair().unwrap();
    drop(client);
    assert!(matches!(
        Files::receive(&server, Uid::current()),
        Err(Error::Frame)
    ));
}

#[test]
fn devices_and_writable_inputs_are_refused() {
    for action in [
        File::open("/dev/null").unwrap(),
        tempfile::tempfile().unwrap(),
    ] {
        let (client, server) = UnixStream::pair().unwrap();
        let mut files = files();
        files.action = action;
        files.send(&client).unwrap();
        assert!(matches!(
            Files::receive(&server, Uid::current()),
            Err(Error::File(0))
        ));
    }
}
