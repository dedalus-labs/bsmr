//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies exact Go SDK selection, lock ownership, and acquisition ownership.

use std::fs;

use bsmr_core::fs::project::ProjectRoot;
use bsmr_fs::paths::abs_norm_path::AbsNormPathBuf;
use bsmr_fs::paths::abs_path::AbsPath;

use crate::commands::go::toolchains_directory;
use crate::commands::go_toolchain::GoToolchainError;
use crate::commands::go_toolchain::acquired_go;
use crate::commands::go_toolchain::configure;
use crate::commands::go_toolchain::select_release;
use crate::commands::go_toolchain::validate_acquisition_owners;
use crate::commands::go_toolchain::write_lock;
use crate::commands::init::set_up_project;

const RELEASES: &[u8] = br#"
[
  {
    "version": "go1.27rc1",
    "stable": false,
    "files": []
  },
  {
    "version": "go1.25.9",
    "stable": true,
    "files": []
  },
  {
    "version": "go1",
    "stable": true,
    "files": []
  },
  {
    "version": "go1.26.5",
    "stable": true,
    "files": [
      {"filename":"go1.26.5.darwin-amd64.tar.gz","os":"darwin","arch":"amd64","version":"go1.26.5","sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","size":11,"kind":"archive"},
      {"filename":"go1.26.5.darwin-arm64.tar.gz","os":"darwin","arch":"arm64","version":"go1.26.5","sha256":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","size":12,"kind":"archive"},
      {"filename":"go1.26.5.linux-amd64.tar.gz","os":"linux","arch":"amd64","version":"go1.26.5","sha256":"cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc","size":13,"kind":"archive"},
      {"filename":"go1.26.5.linux-arm64.tar.gz","os":"linux","arch":"arm64","version":"go1.26.5","sha256":"dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd","size":14,"kind":"archive"}
    ]
  }
]
"#;

/// Confirms the default is the newest stable release, never a prerelease.
#[test]
fn selects_latest_stable_release() {
    let lock = select_release(RELEASES, None).expect("stable release");

    assert_eq!(lock.version(), "1.26.5");
    assert_eq!(lock.archives().len(), 4);
}

/// Confirms initial stable releases without a patch component remain valid exact SDKs.
#[test]
fn selects_patchless_stable_release() {
    let releases = String::from_utf8(RELEASES.to_vec())
        .expect("UTF-8 fixture")
        .replace("1.26.5", "1.27");

    let lock = select_release(releases.as_bytes(), None).expect("patchless stable release");

    assert_eq!(lock.version(), "1.27");
}

/// Confirms explicit versions resolve exactly and require every supported host archive.
#[test]
fn selects_exact_complete_release() {
    let lock = select_release(RELEASES, Some("go1.26.5")).expect("exact release");

    assert_eq!(lock.version(), "1.26.5");
    assert_eq!(lock.archives()[0].sha256(), "a".repeat(64));
    assert_eq!(lock.archives()[3].sha256(), "d".repeat(64));
}

/// Invariant: the lock is the only file `bsmr go toolchain` commits.
///
/// Package evaluation lowers the lock into `toolchains//` targets, so no generated build file
/// or Starlark definition exists to drift from it or to claim a package another source defines.
///
/// Witness:
/// in a root that Cargo and pnpm define, writing the lock adds exactly `.bsmr-go-toolchain.json`,
/// which records every supported host archive.
#[test]
fn writes_only_the_lock() {
    let root = tempfile::tempdir().expect("temporary repository");
    fs::write(root.path().join("Cargo.toml"), "[workspace]\n").expect("Cargo manifest");
    fs::write(root.path().join("package.json"), "{}\n").expect("pnpm manifest");
    let lock = select_release(RELEASES, None).expect("release");

    write_lock(root.path(), &lock, false).expect("lock");

    let mut entries = fs::read_dir(root.path())
        .expect("repository listing")
        .map(|entry| {
            entry
                .expect("entry")
                .file_name()
                .into_string()
                .expect("UTF-8")
        })
        .collect::<Vec<_>>();
    entries.sort();
    assert_eq!(
        entries,
        [".bsmr-go-toolchain.json", "Cargo.toml", "package.json"]
    );
    let written = fs::read_to_string(root.path().join(".bsmr-go-toolchain.json")).expect("lock");
    assert!(written.contains("go1.26.5.darwin-amd64.tar.gz"));
    assert!(written.contains("go1.26.5.linux-arm64.tar.gz"));
}

/// Invariant: the generator replaces only a lock that carries its exact ownership field.
///
/// A quoted marker elsewhere in a JSON file must not let the generator overwrite it.
///
/// Witness:
/// a lock whose `note` quotes the generator name is refused as user-owned.
#[test]
fn rejects_forged_lock_marker() {
    let root = tempfile::tempdir().expect("temporary repository");
    fs::write(
        root.path().join(".bsmr-go-toolchain.json"),
        r#"{"note":"bsmr go toolchain"}"#,
    )
    .expect("forged lock");
    let lock = select_release(RELEASES, None).expect("release");

    let error = write_lock(root.path(), &lock, false).expect_err("forged marker must fail closed");

    assert!(matches!(error, GoToolchainError::UserOwned(_)));
}

/// Confirms release metadata cannot inject an invalid digest into generated Starlark.
#[test]
fn rejects_malformed_release_digest() {
    let releases = String::from_utf8(RELEASES.to_vec())
        .expect("UTF-8 fixture")
        .replace(&"a".repeat(64), "not-a-digest");

    let error = select_release(releases.as_bytes(), Some("1.26.5"))
        .expect_err("malformed digest must fail closed");

    assert!(error.to_string().contains("SHA-256"));
}

/// Invariant: check mode is offline and never rewrites a lock that drifted from its form.
///
/// Witness:
/// a lock with a trailing edit fails the check as stale and keeps the edit.
#[test]
fn check_detects_lock_drift() {
    let root = tempfile::tempdir().expect("temporary repository");
    let lock = select_release(RELEASES, None).expect("release");
    write_lock(root.path(), &lock, false).expect("lock");
    let path = root.path().join(".bsmr-go-toolchain.json");
    let mut drift = fs::read_to_string(&path).expect("lock");
    drift.push('\n');
    fs::write(&path, &drift).expect("drift");

    let error = write_lock(root.path(), &lock, true).expect_err("must detect drift");

    assert!(matches!(error, GoToolchainError::Stale(_)));
    assert_eq!(fs::read_to_string(&path).expect("lock"), drift);
}

/// Confirms a partially replaced SDK and bootstrap wrapper cannot pass acquisition checks.
#[test]
fn rejects_mismatched_bootstrap_acquisition() {
    let root = tempfile::tempdir().expect("temporary repository");
    let sdk = root.path().join("toolchains/.bsmr-go-sdk");
    let tools = root.path().join("toolchains/.bsmr-go-tools");
    fs::create_dir_all(sdk.join("bin")).expect("SDK directory");
    fs::create_dir_all(&tools).expect("tools directory");
    fs::write(sdk.join("VERSION"), "go1.26.5\n").expect("SDK version");
    fs::write(sdk.join("bin/go"), []).expect("Go executable");
    fs::write(tools.join("go_wrapper"), []).expect("wrapper executable");
    let lock = select_release(RELEASES, None).expect("release");
    let lock_value = serde_json::to_value(&lock).expect("serialized lock");
    let host_os = if std::env::consts::OS == "macos" {
        "darwin"
    } else {
        std::env::consts::OS
    };
    let host_arch = if std::env::consts::ARCH == "aarch64" {
        "arm64"
    } else {
        "amd64"
    };
    let archive = lock_value["archives"]
        .as_array()
        .expect("archives")
        .iter()
        .find(|archive| archive["os"] == host_os && archive["arch"] == host_arch)
        .expect("host archive");
    let metadata = serde_json::json!({
        "generated_by": "bsmr go toolchain",
        "state": "acquired",
        "version": "1.26.5",
        "os": host_os,
        "arch": host_arch,
        "sha256": archive["sha256"],
    });
    fs::write(
        sdk.join(".bsmr-metadata.json"),
        serde_json::to_vec(&metadata).expect("SDK metadata"),
    )
    .expect("SDK metadata file");
    let mut stale_tools = metadata;
    stale_tools["state"] = serde_json::Value::String("acquiring".to_owned());
    fs::write(
        tools.join(".bsmr-metadata.json"),
        serde_json::to_vec(&stale_tools).expect("tools metadata"),
    )
    .expect("tools metadata file");

    let error = acquired_go(&root.path().join("toolchains"), &lock)
        .expect_err("must reject partial acquisition");

    assert!(matches!(error, GoToolchainError::NotAcquired));
}

/// Confirms a substring collision cannot claim ownership of an SDK directory.
#[test]
fn rejects_forged_acquisition_marker() {
    let root = tempfile::tempdir().expect("temporary repository");
    let sdk = root.path().join("toolchains/.bsmr-go-sdk");
    fs::create_dir_all(&sdk).expect("SDK directory");
    fs::write(
        sdk.join(".bsmr-metadata.json"),
        r#"{"note":"bsmr go toolchain"}"#,
    )
    .expect("forged ownership marker");

    let error = validate_acquisition_owners(&root.path().join("toolchains"))
        .expect_err("forgery must fail closed");

    assert!(matches!(error, GoToolchainError::UserOwned(_)));
}

/// Confirms ordinary acquisition preserves the exact lock without release-metadata access.
#[test]
fn reacquires_existing_lock_without_resolving_latest() {
    let root = tempfile::tempdir().expect("temporary repository");
    let expected = select_release(RELEASES, Some("1.26.5")).expect("release");
    write_lock(root.path(), &expected, false).expect("lock");

    let actual = futures::executor::block_on(configure(root.path(), None, false, false))
        .expect("existing lock");

    assert_eq!(actual, expected);
}

/// Confirms `tool` roots are requested only from SDKs that define the meta-pattern.
#[test]
fn gates_tool_directives_on_sdk_version() {
    let supported = select_release(RELEASES, Some("1.26.5")).expect("release");
    let releases = String::from_utf8(RELEASES.to_vec())
        .expect("UTF-8 fixture")
        .replace("1.26.5", "1.23.4");
    let unsupported = select_release(releases.as_bytes(), Some("1.23.4")).expect("release");

    assert!(supported.supports_tool_directives().expect("valid version"));
    assert!(
        !unsupported
            .supports_tool_directives()
            .expect("valid version")
    );
}

/// Invariant: acquisition lands in the directory of the package `toolchains//` names.
///
/// The injected toolchain reads `.bsmr-go-sdk` and `.bsmr-go-tools` relative to that package.
/// `bsmr init` aliases `toolchains` to the root cell, so a new project acquires at its root.
///
/// Witness:
/// the configuration `bsmr init` writes resolves `toolchains` to the project root.
#[test]
fn resolves_toolchains_to_the_root_in_an_init_project() {
    let directory = tempfile::tempdir().expect("temporary repository");
    let path = AbsNormPathBuf::new(directory.path().canonicalize().expect("canonical root"))
        .expect("absolute root");
    set_up_project(AbsPath::new(&path).expect("absolute root"), false, true)
        .expect("initialized project");
    let project_root = ProjectRoot::new(path.clone()).expect("project root");

    let toolchains = futures::executor::block_on(toolchains_directory(&project_root))
        .expect("configured toolchains");

    assert_eq!(toolchains, path.as_path());
}
