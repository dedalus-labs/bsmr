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
    let graph = GoGraph::from_go_list(PACKAGE_GRAPH.as_bytes(), Path::new("/repo"), "")
        .expect("valid graph");

    assert_eq!(graph.packages().len(), 2);
    assert_eq!(graph.packages()[0].import_path(), "example.com/repo/lib");
    assert_eq!(graph.packages()[1].dependencies(), ["//lib:lib"]);
    assert_eq!(graph.packages()[1].target_name(), "bin");
}

/// Invariant: labels are cell-relative, so a module synchronized below its cell root
/// names every target through that root's package path, and a package's own label
/// matches the label its consumers depend on.
///
/// Witness: synchronizing `/repo` as cell package `tools`, `cmd/app` depends on
/// `//tools/lib:lib`, and `lib` still orders before `cmd/app` although its import
/// path sorts after it.
#[test]
fn invariant_labels_start_at_the_cell_package() {
    let graph = GoGraph::from_go_list(PACKAGE_GRAPH.as_bytes(), Path::new("/repo"), "tools")
        .expect("valid graph");

    assert_eq!(graph.packages()[0].import_path(), "example.com/repo/lib");
    assert_eq!(graph.packages()[1].dependencies(), ["//tools/lib:lib"]);
}

/// Confirms module-cache dependencies fail instead of silently escaping the repository.
#[test]
fn rejects_non_vendored_dependencies() {
    let error = GoGraph::from_go_list(EXTERNAL_GRAPH.as_bytes(), Path::new("/repo"), "")
        .expect_err("external package must be vendored");

    assert!(error.to_string().contains("go mod vendor"));
    assert!(error.to_string().contains("example.com/external/pkg"));
}

/// Confirms an impossible package cycle fails at the graph boundary.
#[test]
fn rejects_package_cycles() {
    let error = GoGraph::from_go_list(CYCLE_GRAPH.as_bytes(), Path::new("/repo"), "")
        .expect_err("cycle must fail");

    assert!(error.to_string().contains("cycle"));
}

/// Confirms package metadata cannot reference sources outside its package directory.
#[test]
fn rejects_unsafe_source_paths() {
    let graph = r#"{"Dir":"/repo/lib","ImportPath":"example.com/repo/lib","Name":"lib","GoFiles":["../secret.go"]}"#;
    let error = GoGraph::from_go_list(graph.as_bytes(), Path::new("/repo"), "")
        .expect_err("parent traversal must fail");

    assert!(error.to_string().contains("unsafe source path"));
}

/// Confirms dependency tests are ignored while selected external tests are normalized.
#[test]
fn lowers_only_selected_external_tests() {
    let dependency = r#"{"Dir":"/repo/vendor/example.com/dep","ImportPath":"example.com/dep","Name":"dep","DepOnly":true,"TestGoFiles":["dep_internal_test.go"],"XTestGoFiles":["dep_test.go"],"TestImports":["example.com/test-only"]}"#;
    let graph = GoGraph::from_go_list(dependency.as_bytes(), Path::new("/repo"), "")
        .expect("dependency tests are not selected");
    assert!(graph.packages()[0].test_files().is_empty());
    assert!(graph.packages()[0].test_dependencies().is_empty());

    let selected = r#"
{"Dir":"/repo/pkg","ImportPath":"example.com/repo/pkg","Name":"pkg","GoFiles":["pkg.go"],"XTestGoFiles":["pkg_test.go"],"XTestImports":["example.com/repo/helper"],"XTestEmbedFiles":["fixture.txt"]}
{"Dir":"/repo/helper","ImportPath":"example.com/repo/helper","Name":"helper","GoFiles":["helper.go"]}
"#;
    let graph = GoGraph::from_go_list(selected.as_bytes(), Path::new("/repo"), "")
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
    let error = GoGraph::from_go_list(graph.as_bytes(), Path::new("/repo"), "")
        .expect_err("Objective-C sources are unsupported");

    assert!(error.to_string().contains("unsupported source files"));
    assert!(error.to_string().contains("native.m"));
}

const MODULE_GRAPH: &str = r#"
{"Dir":"/repo/cmd/app","ImportPath":"example.com/repo/cmd/app","Name":"main","GoFiles":["main.go"],"Imports":["example.com/dep/api","example.com/repo/lib","fmt"],"Module":{"Path":"example.com/repo","Main":true}}
{"Dir":"/repo/lib","ImportPath":"example.com/repo/lib","Name":"lib","GoFiles":["lib.go"],"Imports":["example.com/leaf"],"TestImports":["example.com/testonly"],"Module":{"Path":"example.com/repo","Main":true}}
{"Dir":"/repo/vendor/example.com/dep/api","ImportPath":"example.com/dep/api","Name":"api","GoFiles":["api.go"],"Imports":["example.com/leaf"],"DepOnly":true,"Module":{"Path":"example.com/dep","Version":"v1.2.0"}}
{"Dir":"/repo/vendor/example.com/leaf","ImportPath":"example.com/leaf","Name":"leaf","GoFiles":["leaf.go"],"DepOnly":true,"Module":{"Path":"example.com/leaf","Version":"v0.1.0","Replace":{"Path":"example.com/fork","Version":"v0.1.1"}}}
{"Dir":"/repo/vendor/example.com/testonly","ImportPath":"example.com/testonly","Name":"testonly","GoFiles":["testonly.go"],"DepOnly":true,"Module":{"Path":"example.com/testonly","Version":"v3.0.0"}}
{"Dir":"/goroot/src/fmt","ImportPath":"fmt","Name":"fmt","Goroot":true,"Standard":true}
"#;

/// Invariant: an executable's module lines are those `go build` records in
/// `debug.BuildInfo`: its own module as `mod`, `(devel)` when Go reports no version,
/// then each other module reachable through production imports as `dep` in path
/// order, a replaced module followed by its `=>` line, and no sums under vendoring.
///
/// Witness: `cmd/app` reaches `example.com/leaf` through two paths and
/// `example.com/testonly` only through a test import; the first appears once with its
/// replacement, the second not at all, and the library records no module lines.
#[test]
fn invariant_executable_records_linked_modules() {
    let graph = GoGraph::from_go_list(MODULE_GRAPH.as_bytes(), Path::new("/repo"), "")
        .expect("valid graph");
    let binary = graph
        .packages()
        .iter()
        .find(|package| package.import_path() == "example.com/repo/cmd/app")
        .expect("binary package");
    let library = graph
        .packages()
        .iter()
        .find(|package| package.import_path() == "example.com/repo/lib")
        .expect("library package");

    assert_eq!(
        binary.modules(),
        [
            "mod\texample.com/repo\t(devel)\t",
            "dep\texample.com/dep\tv1.2.0\t",
            "dep\texample.com/leaf\tv0.1.0",
            "=>\texample.com/fork\tv0.1.1\t",
        ]
    );
    assert!(library.modules().is_empty());
}

/// Invariant: a tool built from a vendored module records that module, with its
/// version, as the main module, matching `go build` of a `tool` directive.
///
/// Witness: `example.com/greeter/cmd/greet` from `example.com/greeter v1.0.0` in the
/// `example.com/tools` module records `mod example.com/greeter v1.0.0`.
#[test]
fn invariant_tool_records_its_module_as_main() {
    let graph = r#"{"Dir":"/repo/vendor/example.com/greeter/cmd/greet","ImportPath":"example.com/greeter/cmd/greet","Name":"main","GoFiles":["main.go"],"Module":{"Path":"example.com/greeter","Version":"v1.0.0"}}"#;
    let graph =
        GoGraph::from_go_list(graph.as_bytes(), Path::new("/repo"), "").expect("valid graph");

    assert_eq!(
        graph.packages()[0].modules(),
        ["mod\texample.com/greeter\tv1.0.0\t"]
    );
}
