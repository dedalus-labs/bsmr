//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies generated Go manifests and their ownership boundary.

use std::fs;

use crate::commands::go_graph::GoGraph;
use crate::commands::go_manifest::GoManifestError;
use crate::commands::go_manifest::SyncMode;
use crate::commands::go_manifest::render_manifest;
use crate::commands::go_manifest::sync_manifests;

/// Builds SDK metadata rooted at the provided temporary repository.
fn graph(root: &std::path::Path) -> GoGraph {
    let display = root.display();
    let json = format!(
        "{{\"Dir\":\"{display}/lib\",\"ImportPath\":\"example.com/repo/lib\",\"Name\":\"lib\",\"GoFiles\":[\"lib.go\"],\"TestGoFiles\":[\"lib_test.go\"]}}\n\
         {{\"Dir\":\"{display}/cmd/app\",\"ImportPath\":\"example.com/repo/cmd/app\",\"Name\":\"main\",\"GoFiles\":[\"main.go\"],\"Imports\":[\"example.com/repo/lib\"]}}\n"
    );
    GoGraph::from_go_list(json.as_bytes(), root).expect("valid graph")
}

/// Builds SDK metadata for a package with an external-package test.
fn external_test_graph(root: &std::path::Path) -> GoGraph {
    let display = root.display();
    let json = format!(
        "{{\"Dir\":\"{display}/lib\",\"ImportPath\":\"example.com/repo/lib\",\"Name\":\"lib\",\"GoFiles\":[\"lib.go\"],\"XTestGoFiles\":[\"external_test.go\"],\"XTestImports\":[\"example.com/repo/lib\"]}}\n"
    );
    GoGraph::from_go_list(json.as_bytes(), root).expect("valid external test graph")
}

/// Builds SDK metadata for a package that includes a C implementation and local header.
fn cgo_graph(root: &std::path::Path) -> GoGraph {
    let display = root.display();
    let json = format!(
        "{{\"Dir\":\"{display}/native\",\"ImportPath\":\"example.com/repo/native\",\"Name\":\"native\",\"CgoFiles\":[\"native.go\"],\"CFiles\":[\"native.c\"],\"HFiles\":[\"native.h\"]}}\n"
    );
    GoGraph::from_go_list(json.as_bytes(), root).expect("valid cgo graph")
}

/// Builds SDK metadata for a package at the repository root.
fn root_graph(root: &std::path::Path) -> GoGraph {
    let display = root.display();
    let json = format!(
        "{{\"Dir\":\"{display}\",\"ImportPath\":\"example.com/repo\",\"Name\":\"main\",\"GoFiles\":[\"main.go\"]}}\n"
    );
    GoGraph::from_go_list(json.as_bytes(), root).expect("valid root graph")
}

/// Confirms native metadata renders conventional Go rules without a handwritten DSL.
#[test]
fn renders_library_binary_and_test_targets() {
    let root = tempfile::tempdir().expect("temporary repository");
    let graph = graph(root.path());
    let library = &graph.packages()[0];
    let binary = &graph.packages()[1];

    let library_manifest =
        render_manifest(library, &["integration".to_owned()], false).expect("library manifest");
    assert!(library_manifest.contains("go_library("));
    assert!(library_manifest.contains("name = \"lib\""));
    assert!(library_manifest.contains("go_test("));
    assert!(library_manifest.contains("target_under_test = \":lib\""));
    assert!(library_manifest.contains("build_tags = [\n        \"integration\","));
    assert!(
        !library_manifest
            .split("go_test(")
            .next()
            .expect("library rule")
            .contains("build_tags")
    );
    assert!(library_manifest.contains("override_cgo_enabled = False"));

    let binary_manifest =
        render_manifest(binary, &["integration".to_owned()], false).expect("binary manifest");
    assert!(binary_manifest.contains("go_binary("));
    assert!(binary_manifest.contains("build_tags = [\n        \"integration\","));
    assert!(binary_manifest.contains("cgo_enabled = False"));
    assert!(binary_manifest.contains("deps = [\n        \"//lib:lib\","));
}

/// Confirms native external tests become a separate conventional Go test package.
#[test]
fn renders_external_test_target() {
    let root = tempfile::tempdir().expect("temporary repository");
    let graph = external_test_graph(root.path());

    let manifest = render_manifest(&graph.packages()[0], &[], false).expect("manifest");

    assert!(manifest.contains("name = \"external_test\""));
    assert!(manifest.contains("package_name = \"example.com/repo/lib_test\""));
    assert!(manifest.contains("srcs = [\n        \"external_test.go\","));
    assert!(manifest.contains("deps = [\n        \"//lib:lib\","));
}

/// Confirms cgo preserves Go's package-local quoted-header lookup semantics.
#[test]
fn invariant_cgo_headers_are_addressable_by_package_local_name() {
    let root = tempfile::tempdir().expect("temporary repository");
    let graph = cgo_graph(root.path());

    let manifest = render_manifest(&graph.packages()[0], &[], true).expect("cgo manifest");

    assert!(manifest.contains("\"native.c\""));
    assert!(manifest.contains("\"native.go\""));
    assert!(manifest.contains("\"native.h\""));
    assert!(manifest.contains("header_namespace = \"\""));
    assert!(manifest.contains("override_cgo_enabled = True"));
}

/// Confirms synchronization is idempotent and refuses user-owned manifests.
#[test]
fn synchronizes_owned_manifests_only() {
    let root = tempfile::tempdir().expect("temporary repository");
    let graph = graph(root.path());
    let first = sync_manifests(
        root.path(),
        &graph,
        "BUILD.bsmr",
        &[],
        false,
        SyncMode::Write,
    )
    .expect("initial sync");
    assert_eq!(first.written(), 2);

    let checked = sync_manifests(
        root.path(),
        &graph,
        "BUILD.bsmr",
        &[],
        false,
        SyncMode::Check,
    )
    .expect("generated files are current");
    assert_eq!(checked.written(), 0);

    fs::write(root.path().join("lib/BUILD.bsmr"), "# human owned\n")
        .expect("replace generated manifest");
    let error = sync_manifests(
        root.path(),
        &graph,
        "BUILD.bsmr",
        &[],
        false,
        SyncMode::Write,
    )
    .expect_err("human-owned manifest must not be overwritten");
    assert!(error.to_string().contains("refusing to overwrite"));
}

/// Confirms a user-owned ownership index cannot authorize generated-file deletion.
#[test]
fn rejects_user_owned_manifest_index() {
    let root = tempfile::tempdir().expect("temporary repository");
    fs::write(root.path().join(".bsmr-go-manifests"), "pkg/BUCK\n").expect("user-owned index");

    let error = sync_manifests(
        root.path(),
        &graph(root.path()),
        "BUCK",
        &[],
        false,
        SyncMode::Write,
    )
    .expect_err("user-owned index must fail closed");

    assert!(matches!(error, GoManifestError::UserOwned(_)));
}

/// Confirms native sync may replace only the byte-identical scaffold emitted by init.
#[test]
fn migrates_untouched_init_manifest() {
    let root = tempfile::tempdir().expect("temporary repository");
    fs::write(
        root.path().join("BUCK"),
        crate::commands::init::LEGACY_ROOT_BUCK,
    )
    .expect("init manifest");

    sync_manifests(
        root.path(),
        &root_graph(root.path()),
        "BUCK",
        &[],
        false,
        SyncMode::Write,
    )
    .expect("migrate init manifest");

    assert!(
        fs::read_to_string(root.path().join("BUCK"))
            .expect("generated manifest")
            .contains("go_binary(")
    );
}
