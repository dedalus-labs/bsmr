//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Hold one trusted runtime snapshot for all privileged qualification jobs.

use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Result, ensure};
use bsmr_native::runtime::Runtime;

/// Drop the captured runtime before retiring its protected parent directory.
pub(crate) struct Image {
    runtime: Runtime,
    launcher: PathBuf,
    _state: tempfile::TempDir,
}

impl Image {
    /// Prepare a root-owned seed once, then exercise actual snapshot capture and sharing.
    pub(crate) fn new(compiler: &Path) -> Result<Self> {
        let state = tempfile::tempdir_in("/private/var/root")?;
        let seed = tempfile::tempdir_in(state.path())?;
        crate::filesystem::seed(seed.path(), compiler)?;
        let launcher = state.path().join("launcher");
        fs::copy(std::env::current_exe()?, &launcher)?;
        fs::set_permissions(&launcher, fs::Permissions::from_mode(0o555))?;
        let started = Instant::now();
        let runtime = Runtime::capture(seed.path(), state.path(), &launcher)?;
        let capture_ms = started.elapsed().as_secs_f64() * 1000.0;
        let first = runtime.instantiate()?;
        let second = runtime.instantiate()?;
        let name = "toolchain/bin/rustc";
        ensure!(
            fs::metadata(first.path().join(name))?.ino()
                == fs::metadata(second.path().join(name))?.ino()
        );
        ensure!(
            fs::metadata(first.path().join("tmp"))?.ino()
                != fs::metadata(second.path().join("tmp"))?.ino()
        );
        fs::write(seed.path().join("source.rs"), b"changed after capture")?;
        ensure!(fs::read(first.path().join("source.rs"))? == b"pub fn answer() -> u64 { 42 }\n");
        println!(
            "{}",
            serde_json::json!({"case":"runtime_snapshot", "capture_ms":capture_ms, "shared_inodes":true, "private_scratch":true, "digest":runtime.digest()})
        );
        Ok(Self {
            runtime,
            launcher,
            _state: state,
        })
    }

    /// Create private directories that share only immutable runtime file inodes.
    pub(crate) fn root(&self) -> Result<tempfile::TempDir> {
        Ok(self.runtime.instantiate()?)
    }

    /// Return the administrator-owned launcher bound into the runtime fingerprint.
    pub(crate) fn launcher(&self) -> &Path {
        &self.launcher
    }

    /// Return the actual runtime identity supplied in each test request.
    pub(crate) fn digest(&self) -> &str {
        self.runtime.digest()
    }
}
