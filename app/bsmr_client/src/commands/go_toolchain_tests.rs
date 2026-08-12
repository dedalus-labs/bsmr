//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies exact Go SDK selection and generated toolchain ownership.

use std::fs;

use crate::commands::go_toolchain::GoToolchainError;
use crate::commands::go_toolchain::acquired_go;
use crate::commands::go_toolchain::configure;
use crate::commands::go_toolchain::prepare_acquisition;
use crate::commands::go_toolchain::select_release;
use crate::commands::go_toolchain::write_configuration;

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

/// Confirms native setup owns the generated IR while preserving the init migration.
#[test]
fn writes_owned_cross_host_toolchain_configuration() {
    let root = tempfile::tempdir().expect("temporary repository");
    fs::create_dir(root.path().join("toolchains")).expect("toolchains directory");
    fs::write(
        root.path().join("toolchains/BUILD.bsmr"),
        crate::commands::init::INITIAL_TOOLCHAINS_MANIFEST,
    )
    .expect("initial toolchains");
    let lock = select_release(RELEASES, None).expect("release");

    write_configuration(root.path(), &lock, false).expect("generated configuration");

    let manifest =
        fs::read_to_string(root.path().join("toolchains/BUILD.bsmr")).expect("toolchain manifest");
    let definition = fs::read_to_string(root.path().join("toolchains/bsmr_go_toolchain.bzl"))
        .expect("toolchain definition");
    assert!(manifest.contains("system_demo_toolchains(include_go = False)"));
    assert!(manifest.contains("bsmr_go_toolchains()"));
    assert!(definition.contains("go1.26.5.darwin-amd64.tar.gz"));
    assert!(definition.contains("go1.26.5.linux-arm64.tar.gz"));
    assert!(definition.contains(&format!("sha256 = \"{}\"", "a".repeat(64))));
    assert!(definition.contains("size_bytes = 11"));
    assert!(definition.contains("name = \"go_sdk_archive\""));
    assert!(definition.contains("go_root = \".bsmr-go-sdk\""));
    assert!(definition.contains("go_wrapper = \":go_bootstrap_wrapper\""));
    assert!(definition.contains("env_go_experiment = [\"none\"]"));
}

/// Confirms native setup cannot replace a repository's custom toolchain graph.
#[test]
fn refuses_user_owned_toolchain_configuration() {
    let root = tempfile::tempdir().expect("temporary repository");
    fs::create_dir(root.path().join("toolchains")).expect("toolchains directory");
    fs::write(
        root.path().join("toolchains/BUILD.bsmr"),
        "custom_toolchain()\n",
    )
    .expect("custom toolchain");
    let lock = select_release(RELEASES, None).expect("release");

    let error = write_configuration(root.path(), &lock, false).expect_err("must fail closed");

    assert!(matches!(error, GoToolchainError::UserOwned(_)));
}

/// Confirms a quoted generator marker cannot claim a custom toolchain file.
#[test]
fn rejects_forged_toolchain_marker() {
    let root = tempfile::tempdir().expect("temporary repository");
    fs::create_dir(root.path().join("toolchains")).expect("toolchains directory");
    fs::write(
        root.path().join("toolchains/BUILD.bsmr"),
        "custom_toolchain()\n# Generated by `bsmr go toolchain`; DO NOT EDIT.\n",
    )
    .expect("forged toolchain marker");
    let lock = select_release(RELEASES, None).expect("release");

    let error =
        write_configuration(root.path(), &lock, false).expect_err("forged marker must fail closed");

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

/// Confirms check mode is offline and detects generated-definition drift.
#[test]
fn check_detects_toolchain_drift() {
    let root = tempfile::tempdir().expect("temporary repository");
    fs::create_dir(root.path().join("toolchains")).expect("toolchains directory");
    fs::write(
        root.path().join("toolchains/BUILD.bsmr"),
        crate::commands::init::INITIAL_TOOLCHAINS_MANIFEST,
    )
    .expect("initial toolchains");
    let lock = select_release(RELEASES, None).expect("release");
    write_configuration(root.path(), &lock, false).expect("generated configuration");
    let definition = root.path().join("toolchains/bsmr_go_toolchain.bzl");
    let mut drift = fs::read_to_string(&definition).expect("generated definition");
    drift.push_str("# drift\n");
    fs::write(definition, drift).expect("drift");

    let error = write_configuration(root.path(), &lock, true).expect_err("must detect drift");

    assert!(matches!(error, GoToolchainError::Stale(_)));
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

    let error = acquired_go(root.path(), &lock).expect_err("must reject partial acquisition");

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
    let lock = select_release(RELEASES, None).expect("release");

    let error = prepare_acquisition(root.path(), &lock).expect_err("forgery must fail closed");

    assert!(matches!(error, GoToolchainError::UserOwned(_)));
}

/// Confirms ordinary acquisition preserves the exact lock without release-metadata access.
#[test]
fn reacquires_existing_lock_without_resolving_latest() {
    let root = tempfile::tempdir().expect("temporary repository");
    let expected = select_release(RELEASES, Some("1.26.5")).expect("release");
    write_configuration(root.path(), &expected, false).expect("generated configuration");

    let actual = futures::executor::block_on(configure(root.path(), None, false, false))
        .expect("existing lock");

    assert_eq!(actual, expected);
}
