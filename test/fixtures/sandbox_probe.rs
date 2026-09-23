//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Exercises host-containment invariants from inside a Firecracker guest.

use std::fs;
use std::io::Read;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::fs::symlink;
use std::path::Path;
use std::time::Duration;

/// Verifies isolation or deliberately remains alive for lifecycle tests.
fn main() {
    if std::env::args().nth(1).as_deref() == Some("hang") {
        loop {
            std::thread::sleep(Duration::from_secs(1));
        }
    }
    if std::env::args().nth(1).as_deref() == Some("exit7") {
        eprintln!("requested failure");
        std::process::exit(7);
    }
    assert_eq!(std::env::var("BSMR_PROBE").as_deref(), Ok("exact"));
    assert!(std::env::var_os("HOME").is_none());
    assert!(!Path::new("/bsmr-host-sentinel").exists());
    let interfaces = fs::read_to_string("/proc/net/dev").unwrap();
    assert!(
        interfaces
            .lines()
            .skip(2)
            .all(|line| line.split(':').next().unwrap().trim() == "lo")
    );
    fs::create_dir("out").unwrap();
    if std::env::args().nth(1).as_deref() == Some("random") {
        let mut random = [0u8; 32];
        fs::File::open("/dev/urandom")
            .unwrap()
            .read_exact(&mut random)
            .unwrap();
        fs::write("out/random", random).unwrap();
        return;
    }
    fs::create_dir("out/empty").unwrap();
    fs::write("out/result", b"isolated\n").unwrap();
    fs::write("out/unicode-\u{2603}", b"snow\n").unwrap();
    fs::write("out/executable", b"#!/bin/sh\n").unwrap();
    fs::set_permissions("out/executable", fs::Permissions::from_mode(0o755)).unwrap();
    symlink("result", "out/link").unwrap();
    fs::write("undeclared", b"never import me\n").unwrap();
    eprintln!("sandbox probe stderr");
    println!("sandbox probe passed");
}
