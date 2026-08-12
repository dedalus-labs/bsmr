//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies deterministic lowering from Go SDK metadata into Bessemer packages.

use std::path::Path;

use crate::commands::go_graph::GoGraph;

const PACKAGE_GRAPH: &str = r#"
{"Dir":"/repo/lib","ImportPath":"example.com/repo/lib","Name":"lib","GoFiles":["lib.go"],"Imports":["fmt"]}
{"Dir":"/repo/cmd/app","ImportPath":"example.com/repo/cmd/app","Name":"main","GoFiles":["main.go"],"Imports":["example.com/repo/lib"]}
{"Dir":"/goroot/src/fmt","ImportPath":"fmt","Name":"fmt","Goroot":true,"Standard":true}
{"Dir":"/repo/lib","ImportPath":"example.com/repo/lib [example.com/repo/lib.test]","Name":"lib","ForTest":"example.com/repo/lib"}
{"Dir":"/repo/lib","ImportPath":"example.com/repo/lib.test","Name":"main","GoFiles":["/tmp/go-build/testmain.go"],"Imports":["example.com/repo/lib [example.com/repo/lib.test]"]}
"#;

const EXTERNAL_GRAPH: &str = r#"
{"Dir":"/repo/app","ImportPath":"example.com/repo/app","Name":"app","Imports":["example.com/external/pkg"]}
{"Dir":"/gomod/pkg","ImportPath":"example.com/external/pkg","Name":"pkg"}
"#;

const CYCLE_GRAPH: &str = r#"
{"Dir":"/repo/a","ImportPath":"example.com/repo/a","Name":"a","Imports":["example.com/repo/b"]}
{"Dir":"/repo/b","ImportPath":"example.com/repo/b","Name":"b","Imports":["example.com/repo/a"]}
"#;

/// Confirms SDK test variants are discarded and internal imports become labels.
#[test]
fn lowers_sdk_graph_deterministically() {
    let graph =
        GoGraph::from_go_list(PACKAGE_GRAPH.as_bytes(), Path::new("/repo")).expect("valid graph");

    assert_eq!(graph.packages().len(), 2);
    assert_eq!(graph.packages()[0].import_path(), "example.com/repo/lib");
    assert_eq!(graph.packages()[1].dependencies(), ["//lib:lib"]);
    assert_eq!(graph.packages()[1].target_name(), "bin");
}

/// Confirms module-cache dependencies fail instead of silently escaping the repository.
#[test]
fn rejects_non_vendored_dependencies() {
    let error = GoGraph::from_go_list(EXTERNAL_GRAPH.as_bytes(), Path::new("/repo"))
        .expect_err("external package must be vendored");

    assert!(error.to_string().contains("go mod vendor"));
    assert!(error.to_string().contains("example.com/external/pkg"));
}

/// Confirms an impossible package cycle fails at the graph boundary.
#[test]
fn rejects_package_cycles() {
    let error = GoGraph::from_go_list(CYCLE_GRAPH.as_bytes(), Path::new("/repo"))
        .expect_err("cycle must fail");

    assert!(error.to_string().contains("cycle"));
}

/// Confirms package metadata cannot reference sources outside its package directory.
#[test]
fn rejects_unsafe_source_paths() {
    let graph = r#"{"Dir":"/repo/lib","ImportPath":"example.com/repo/lib","Name":"lib","GoFiles":["../secret.go"]}"#;
    let error = GoGraph::from_go_list(graph.as_bytes(), Path::new("/repo"))
        .expect_err("parent traversal must fail");

    assert!(error.to_string().contains("unsafe source path"));
}

/// Confirms dependency tests are ignored while selected external tests are normalized.
#[test]
fn lowers_only_selected_external_tests() {
    let dependency = r#"{"Dir":"/repo/vendor/example.com/dep","ImportPath":"example.com/dep","Name":"dep","DepOnly":true,"TestGoFiles":["dep_internal_test.go"],"XTestGoFiles":["dep_test.go"],"TestImports":["example.com/test-only"]}"#;
    let graph = GoGraph::from_go_list(dependency.as_bytes(), Path::new("/repo"))
        .expect("dependency tests are not selected");
    assert!(graph.packages()[0].test_files().is_empty());
    assert!(graph.packages()[0].test_dependencies().is_empty());

    let selected = r#"
{"Dir":"/repo/pkg","ImportPath":"example.com/repo/pkg","Name":"pkg","GoFiles":["pkg.go"],"XTestGoFiles":["pkg_test.go"],"XTestImports":["example.com/repo/helper"],"XTestEmbedFiles":["fixture.txt"]}
{"Dir":"/repo/helper","ImportPath":"example.com/repo/helper","Name":"helper","GoFiles":["helper.go"]}
"#;
    let graph = GoGraph::from_go_list(selected.as_bytes(), Path::new("/repo"))
        .expect("selected external tests lower into their own package");
    let package = &graph.packages()[1];
    assert_eq!(package.external_test_files(), ["pkg_test.go"]);
    assert_eq!(package.external_test_dependencies(), ["//helper:lib"]);
    assert_eq!(package.external_test_embed_files(), ["fixture.txt"]);
}

/// Confirms source kinds unsupported by the prelude fail during graph import.
#[test]
fn rejects_unsupported_source_kinds() {
    let graph = r#"{"Dir":"/repo/pkg","ImportPath":"example.com/repo/pkg","Name":"pkg","MFiles":["native.m"]}"#;
    let error = GoGraph::from_go_list(graph.as_bytes(), Path::new("/repo"))
        .expect_err("Objective-C sources are unsupported");

    assert!(error.to_string().contains("unsupported source files"));
    assert!(error.to_string().contains("native.m"));
}
