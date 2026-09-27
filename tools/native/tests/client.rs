//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Require the kernel peer to be root before sending any file capabilities.

#![cfg(target_os = "macos")]

use std::fs::File;
use std::time::Duration;

use bsmr_sandbox::native::client::{self, Error};
use bsmr_sandbox::native::files::Files;

#[tokio::test]
async fn ordinary_processes_cannot_impersonate_the_worker() {
    if nix::unistd::Uid::current().is_root() {
        return;
    }
    let root = tempfile::tempdir().unwrap();
    let socket = root.path().join("worker.sock");
    let _listener = std::os::unix::net::UnixListener::bind(&socket).unwrap();
    let input = tempfile::NamedTempFile::new().unwrap();
    let files = Files {
        action: File::open(input.path()).unwrap(),
        input: File::open(input.path()).unwrap(),
        output: tempfile::tempfile().unwrap(),
    };
    assert!(matches!(
        client::execute(&socket, &files, Duration::from_secs(5)).await,
        Err(Error::Peer)
    ));
}
