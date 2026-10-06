//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies native Go command configuration semantics.

use std::fs;

use clap::Parser;

use crate::commands::go::GoCommand;
use crate::commands::go::discover_patterns;
use crate::commands::go::parse_sdk_output;
use crate::commands::go::select_buildfile;
use crate::commands::go::validate_build_tags;

/// Confirms synchronization follows the active Bessemer build-file convention.
#[test]
fn selects_configured_or_canonical_default_buildfile() {
    assert_eq!(select_buildfile(None, None).expect("default"), "BUILD.bsmr");
    assert_eq!(
        select_buildfile(Some(vec!["BUILD.bsmr".to_owned()]), None).expect("v2 name"),
        "BUILD.bsmr"
    );
    assert_eq!(
        select_buildfile(None, Some(vec!["TARGETS".to_owned()])).expect("configured name"),
        "TARGETS"
    );
}

/// Confirms one synchronization cannot silently write multiple manifest schemes.
#[test]
fn rejects_ambiguous_buildfile_configuration() {
    let error = select_buildfile(
        None,
        Some(vec!["BUILD.bsmr".to_owned(), "TARGETS".to_owned()]),
    )
    .expect_err("multiple build files require an override");

    assert!(error.to_string().contains("--buildfile"));
}

/// Confirms tags cannot select a graph that the Bessemer transition cannot represent.
#[test]
fn rejects_unconfigured_build_tags() {
    validate_build_tags(&["integration".to_owned()], &["integration".to_owned()])
        .expect("configured tag");
    let error = validate_build_tags(&["enterprise".to_owned()], &["integration".to_owned()])
        .expect_err("unknown tag must fail before generation");

    assert!(error.to_string().contains("go.allowed_build_tags"));
}

/// Confirms default discovery excludes generated, vendored, and hidden trees.
#[test]
fn discovers_package_roots_without_bsmr_outputs() {
    let root = tempfile::tempdir().expect("temporary repository");
    for directory in ["pkg", "bsmr-out", "vendor", ".hidden", "_tools"] {
        fs::create_dir(root.path().join(directory)).expect("fixture directory");
    }
    fs::write(root.path().join("root.go"), "package root\n").expect("root source");

    assert_eq!(
        discover_patterns(root.path()).expect("package roots"),
        [".", "./pkg/..."]
    );
}

/// Confirms SDK materialization accepts exactly one structured Bessemer output.
#[test]
fn parses_materialized_sdk_output() {
    let stdout = b"BUILD SUCCEEDED\n{\"toolchains//:go_sdk\":\"/tmp/sdk\"}\n";

    assert_eq!(
        parse_sdk_output(stdout).expect("SDK output"),
        std::path::Path::new("/tmp/sdk")
    );
    assert!(parse_sdk_output(b"BUILD SUCCEEDED\n").is_err());
    assert!(parse_sdk_output(b"{\"one\":\"a\",\"two\":\"b\"}\n").is_err());
}

/// Target parsing accepts the SDK-supported tuples and rejects implicit spellings.
#[test]
fn synchronization_accepts_explicit_go_target_tuples() {
    for target in ["linux/amd64", "linux/arm64", "darwin/amd64", "darwin/arm64"] {
        GoCommand::try_parse_from(["go", "sync", "--target", target, "--check"])
            .expect("supported target");
    }
    for target in ["linux", "linux/aarch64", "windows/amd64"] {
        assert!(GoCommand::try_parse_from(["go", "sync", "--target", target]).is_err());
    }
}
