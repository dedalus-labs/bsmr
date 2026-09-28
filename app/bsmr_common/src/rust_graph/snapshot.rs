//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Selects files Cargo can use to discover targets without parsing Rust modules.

use std::path::Path;

/// Keeps a conservative superset of Cargo's conventional target entrypoints.
pub(super) fn inferred(path: &Path) -> bool {
    let Some(parent) = path.parent() else {
        return false;
    };
    let name = path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or_default();
    if name == "build.rs" || (parent.ends_with("src") && matches!(name, "lib.rs" | "main.rs")) {
        return true;
    }
    let target_directory = |directory: &Path| {
        directory.ends_with("src/bin")
            || ["examples", "tests", "benches"]
                .iter()
                .any(|name| directory.ends_with(name))
    };
    (path.extension().is_some_and(|s| s == "rs") && target_directory(parent))
        || (name == "main.rs" && parent.parent().is_some_and(target_directory))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retains_automatic_targets_without_staging_ordinary_modules() {
        for path in [
            "pkg/src/lib.rs",
            "pkg/src/main.rs",
            "pkg/build.rs",
            "pkg/src/bin/tool.rs",
            "pkg/src/bin/tool/main.rs",
            "pkg/examples/demo.rs",
            "pkg/examples/demo/main.rs",
            "pkg/tests/contract.rs",
            "pkg/tests/contract/main.rs",
            "pkg/benches/bench.rs",
            "pkg/benches/bench/main.rs",
        ] {
            assert!(inferred(Path::new(path)), "{path}");
        }
        for path in [
            "pkg/src/module.rs",
            "pkg/src/deep/module.rs",
            "pkg/tests/contract/helper.rs",
            "pkg/src/bin/tool/helper.rs",
            "pkg/assets/input.txt",
        ] {
            assert!(!inferred(Path::new(path)), "{path}");
        }
    }

    #[test]
    fn cargo_discovers_identical_targets_from_reduced_sources() {
        let manifest = "[package]\nname='fixture'\nversion='0.1.0'\nedition='2024'\nbuild='hooks/build.code'\n[lib]\npath='library.code'\n[[bin]]\nname='custom'\npath='tools/custom.code'\n";
        let files = [
            "library.code",
            "tools/custom.code",
            "hooks/build.code",
            "src/main.rs",
            "src/bin/tool.rs",
            "src/bin/nested/main.rs",
            "examples/demo.rs",
            "examples/nested/main.rs",
            "tests/contract.rs",
            "tests/nested/main.rs",
            "benches/bench.rs",
            "benches/nested/main.rs",
            "src/unused.rs",
            "src/bin/nested/helper.rs",
        ];
        let metadata: Vec<serde_json::Value> = [false, true]
            .into_iter()
            .map(|reduced| {
                let root = tempfile::tempdir().unwrap();
                std::fs::write(root.path().join("Cargo.toml"), manifest).unwrap();
                for file in files {
                    if reduced && !inferred(Path::new(file)) {
                        continue;
                    }
                    let path = root.path().join(file);
                    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                    std::fs::write(path, "").unwrap();
                }
                let output = std::process::Command::new(env!("CARGO"))
                    .args(["metadata", "--offline", "--no-deps", "--format-version=1"])
                    .current_dir(root.path())
                    .output()
                    .unwrap();
                assert!(
                    output.status.success(),
                    "{}",
                    String::from_utf8_lossy(&output.stderr)
                );
                let text = String::from_utf8(output.stdout).unwrap();
                serde_json::from_str(&text.replace(root.path().to_str().unwrap(), "WORKSPACE"))
                    .unwrap()
            })
            .collect();
        assert_eq!(metadata[0], metadata[1]);
        assert!(
            metadata[0]["packages"][0]["targets"]
                .as_array()
                .unwrap()
                .len()
                >= 10
        );
    }
}
