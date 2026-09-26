//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Preserve analyzed contents and isolation when staging a native input tree.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use bsmr_common::file_ops::metadata::FileMetadata;
use bsmr_common::file_ops::metadata::Symlink;
use bsmr_common::file_ops::metadata::TrackedFileDigest;
use bsmr_core::fs::project_rel_path::ProjectRelativePath;
use bsmr_directory::directory::entry::DirectoryEntry;
use bsmr_execute::digest_config::DigestConfig;
use bsmr_execute::directory::ActionDirectoryBuilder;
use bsmr_execute::directory::ActionDirectoryMember;
use bsmr_execute::directory::ActionImmutableDirectory;
use bsmr_execute::directory::insert_entry;
use bsmr_execute::directory::insert_file;

use super::Cache;

/// Independent commands must independently verify their source bytes.
fn stage(
    project: &Path,
    root: &Path,
    tree: &ActionImmutableDirectory,
    config: DigestConfig,
) -> bsmr_error::Result<()> {
    Cache::new(root.parent().unwrap())?.stage(project, root, tree, config)
}

/// Declare a real executable input and one alias without inspecting host symlink targets.
fn directory(config: DigestConfig, target: &str) -> ActionImmutableDirectory {
    let mut builder = ActionDirectoryBuilder::empty();
    insert_file(
        &mut builder,
        ProjectRelativePath::new("src/tool").unwrap().to_buf(),
        FileMetadata {
            digest: TrackedFileDigest::from_content(b"original", config.cas_digest_config()),
            is_executable: true,
        },
    )
    .unwrap();
    insert_entry(
        &mut builder,
        ProjectRelativePath::new("alias").unwrap().to_buf(),
        DirectoryEntry::Leaf(ActionDirectoryMember::Symlink(Arc::new(Symlink::new(
            target.into(),
        )))),
    )
    .unwrap();
    builder.fingerprint(config.as_directory_serializer())
}

#[test]
fn invariant_staged_inputs_retain_declared_bytes_and_metadata() {
    let temporary = tempfile::tempdir().unwrap();
    let project = temporary.path().join("project");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/tool"), b"original").unwrap();
    let config = DigestConfig::testing_default();
    let tree = directory(config, "src/tool");
    let staged = temporary.path().join("staged");
    stage(&project, &staged, &tree, config).unwrap();
    assert_eq!(fs::read(staged.join("alias")).unwrap(), b"original");
    assert_eq!(
        fs::read_link(staged.join("alias")).unwrap().to_str(),
        Some("src/tool")
    );
    for path in ["src", "src/tool"] {
        let metadata = fs::metadata(staged.join(path)).unwrap();
        assert_eq!(metadata.permissions().mode() & 0o777, 0o755);
        assert_eq!(metadata.modified().unwrap(), UNIX_EPOCH);
    }
    for contents in [b"mutated!".as_slice(), b"originalextra", b"short"] {
        fs::write(project.join("src/tool"), contents).unwrap();
        assert_eq!(fs::read(staged.join("src/tool")).unwrap(), b"original");
        let other = tempfile::tempdir().unwrap();
        let error = stage(&project, &other.path().join("inputs"), &tree, config).unwrap_err();
        assert!(
            error.to_string().contains("changed after analysis"),
            "{error}"
        );
    }
}

#[test]
fn invariant_input_links_cannot_leave_the_snapshot() {
    let project = tempfile::tempdir().unwrap();
    fs::create_dir(project.path().join("src")).unwrap();
    fs::write(project.path().join("src/tool"), b"original").unwrap();
    let staged = tempfile::tempdir().unwrap();
    let config = DigestConfig::testing_default();
    let tree = directory(config, "../outside");
    let error = stage(project.path(), &staged.path().join("inputs"), &tree, config).unwrap_err();
    assert!(
        error.to_string().contains("escapes the action root"),
        "{error}"
    );
    assert!(!staged.path().join("outside").exists());
}

/// A native source graph is not constrained by the VM archive's entry budget.
#[test]
fn invariant_native_inputs_accept_large_declared_trees() {
    let temporary = tempfile::tempdir().unwrap();
    let config = DigestConfig::testing_default();
    let mut builder = ActionDirectoryBuilder::empty();
    for index in 0..100_001 {
        insert_entry(
            &mut builder,
            ProjectRelativePath::new(&format!("alias{index}"))
                .unwrap()
                .to_buf(),
            DirectoryEntry::Leaf(ActionDirectoryMember::Symlink(Arc::new(Symlink::new(
                "declared".into(),
            )))),
        )
        .unwrap();
    }
    let tree = builder.fingerprint(config.as_directory_serializer());
    let inputs = temporary.path().join("inputs");
    stage(temporary.path(), &inputs, &tree, config).unwrap();
    assert_eq!(fs::read_dir(&inputs).unwrap().count(), 100_001);
    assert_eq!(
        fs::read_link(inputs.join("alias100000")).unwrap(),
        Path::new("declared")
    );
}

/// A command keeps one verified version, even if its mutable origin changes later.
#[test]
fn invariant_shared_snapshots_preserve_analyzed_bytes() {
    use std::os::unix::fs::MetadataExt;
    let temporary = tempfile::tempdir().unwrap();
    let project = temporary.path().join("project");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/tool"), b"original").unwrap();
    let config = DigestConfig::testing_default();
    let tree = directory(config, "src/tool");
    let cache = Cache::new(temporary.path()).unwrap();
    let a = temporary.path().join("a");
    let b = temporary.path().join("b");
    cache.stage(&project, &a, &tree, config).unwrap();
    fs::remove_file(project.join("src/tool")).unwrap();
    cache.stage(&project, &b, &tree, config).unwrap();
    assert_eq!(
        fs::metadata(a.join("src/tool")).unwrap().ino(),
        fs::metadata(b.join("src/tool")).unwrap().ino()
    );
    drop(cache);
    fs::remove_dir_all(a).unwrap();
    assert_eq!(fs::read(b.join("alias")).unwrap(), b"original");
}

#[test]
fn invariant_failed_verification_cannot_seed_a_shared_snapshot() {
    let temporary = tempfile::tempdir().unwrap();
    let project = temporary.path().join("project");
    fs::create_dir_all(project.join("src")).unwrap();
    fs::write(project.join("src/tool"), b"changed!").unwrap();
    let config = DigestConfig::testing_default();
    let tree = directory(config, "src/tool");
    let cache = Cache::new(temporary.path()).unwrap();
    assert!(
        cache
            .stage(&project, &temporary.path().join("bad"), &tree, config)
            .is_err()
    );
    fs::write(project.join("src/tool"), b"original").unwrap();
    let good = temporary.path().join("good");
    cache.stage(&project, &good, &tree, config).unwrap();
    assert_eq!(fs::read(good.join("src/tool")).unwrap(), b"original");
}

#[test]
fn invariant_executable_modes_do_not_alias_shared_bytes() {
    let temporary = tempfile::tempdir().unwrap();
    fs::write(temporary.path().join("input"), b"original").unwrap();
    let config = DigestConfig::testing_default();
    let cache = Cache::new(temporary.path()).unwrap();
    for is_executable in [false, true] {
        let mut builder = ActionDirectoryBuilder::empty();
        insert_file(
            &mut builder,
            ProjectRelativePath::new("input").unwrap().to_buf(),
            FileMetadata {
                digest: TrackedFileDigest::from_content(b"original", config.cas_digest_config()),
                is_executable,
            },
        )
        .unwrap();
        cache
            .stage(
                temporary.path(),
                &temporary.path().join(is_executable.to_string()),
                &builder.fingerprint(config.as_directory_serializer()),
                config,
            )
            .unwrap();
    }
    for (name, mode) in [("false", 0o644), ("true", 0o755)] {
        assert_eq!(
            fs::metadata(temporary.path().join(name).join("input"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            mode
        );
    }
}
