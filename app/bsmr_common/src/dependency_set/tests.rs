//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Verifies DependencySet contract range, verification, update, closure, and receipt invariants.

use std::collections::BTreeMap;

use super::*;
use crate::cas_digest::CasDigestConfig;
use crate::cas_digest::DigestAlgorithm;

const SHA_A: &str = "1111111111111111111111111111111111111111";
const SHA_B: &str = "2222222222222222222222222222222222222222";
const SHA_C: &str = "3333333333333333333333333333333333333333";

const DIGEST_1: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000001";
const DIGEST_2: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000002";
const DIGEST_3: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000003";
const DIGEST_4: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000004";

/// Returns a deterministic SHA-256 CAS digest config for tests.
fn digest_config() -> CasDigestConfig {
    CasDigestConfig::leak_new(vec![DigestAlgorithm::Sha256], None).unwrap()
}

/// Builds a valid [`LockedComponent`] fixture with structured contract requirements.
fn component(
    name: &str,
    revision: &str,
    provides: &[(&str, u64)],
    requires: &[(&str, ContractRange)],
) -> LockedComponent {
    LockedComponent {
        name: name.to_owned(),
        repository: format!("https://example.com/{name}"),
        revision: revision.to_owned(),
        tree: DIGEST_1.to_owned(),
        manifest: DIGEST_2.to_owned(),
        patches: Vec::new(),
        overlay: None,
        provides: provides
            .iter()
            .map(|(k, v)| ((*k).to_owned(), *v))
            .collect(),
        requires: requires
            .iter()
            .map(|(k, v)| ((*k).to_owned(), v.clone()))
            .collect(),
        artifacts: vec![LockedArtifact {
            name: format!("{name}-bin"),
            digest: DIGEST_3.to_owned(),
            reference: None,
        }],
        lockfiles: Vec::new(),
    }
}

#[test]
fn structured_contract_ranges_validate_canonicalize_and_intersect() {
    let cases = [
        (ContractRange::exact(7), [false, true, false]),
        (ContractRange::bounds(Some(6), Some(8)), [true, true, true]),
        (ContractRange::bounds(Some(7), None), [false, true, true]),
        (ContractRange::bounds(None, Some(7)), [true, true, false]),
        (ContractRange::versions(vec![6, 8]), [true, false, true]),
    ];
    let probes = [6u64, 7u64, 8u64];

    for (range, expected) in cases {
        let set = range.to_range_set("example.api").unwrap();
        for (version, want) in probes.into_iter().zip(expected) {
            assert_eq!(set.contains(version), want, "{range} at {version}");
        }
    }

    // Disjoint intersection is empty.
    let left = ContractRange::bounds(Some(1), Some(3))
        .to_range_set("example.api")
        .unwrap();
    let right = ContractRange::bounds(Some(4), Some(6))
        .to_range_set("example.api")
        .unwrap();
    assert!(left.intersection(&right).is_empty());

    let bad_ranges = [
        ContractRange::default(),
        ContractRange::bounds(Some(8), Some(7)),
        ContractRange::versions(vec![]),
        ContractRange {
            min: Some(1),
            max: None,
            versions: Some(vec![1]),
        },
    ];
    for bad in bad_ranges {
        assert!(matches!(
            bad.to_range_set("example.api"),
            Err(DependencySetError::InvalidContractRange { .. })
        ));
    }
    assert!(matches!(
        validate_component_name("BadName"),
        Err(DependencySetError::InvalidComponentName { .. })
    ));
    assert!(matches!(
        validate_contract_name("example..bad"),
        Err(DependencySetError::InvalidContractName { .. })
    ));
    assert!(matches!(
        validate_commit_sha("not-a-sha"),
        Err(DependencySetError::InvalidCommitSha { .. })
    ));
    assert!(matches!(
        validate_sha256_digest("tree", "md5:1234"),
        Err(DependencySetError::InvalidSha256Digest { .. })
    ));
    assert!(matches!(
        validate_target_platform("linux"),
        Err(DependencySetError::InvalidTargetPlatform { .. })
    ));
}

#[test]
fn equivalent_manifests_and_locks_share_canonical_digests() {
    let manifest_a = ComponentManifest::from_toml_str(
        "schema = 1\nname = 'app'\n[provides]\n'example.b' = 2\n'example.a' = 1\n[requires]\n'example.kernel.uapi' = { min = 7, max = 7 }\n",
    )
    .unwrap();
    let manifest_b = ComponentManifest::from_toml_str(
        "# comment\nname = 'app'\nschema = 1\n[requires]\n'example.kernel.uapi' = { versions = [7, 7] }\n[provides]\n'example.a' = 1\n'example.b' = 2\n",
    )
    .unwrap();

    let set_a = DependencySet::from_manifest(manifest_a).unwrap();
    let set_b = DependencySet::from_manifest(manifest_b).unwrap();
    assert_eq!(
        set_a.digest(digest_config()).unwrap(),
        set_b.digest(digest_config()).unwrap()
    );

    let lock_a = DependencyLock {
        schema: 1,
        components: vec![
            component("kernel", SHA_B, &[("example.kernel.uapi", 7)], &[]),
            component(
                "app",
                SHA_A,
                &[],
                &[("example.kernel.uapi", ContractRange::exact(7))],
            ),
        ],
    };
    let lock_b = DependencyLock {
        schema: 1,
        components: vec![
            component(
                "app",
                SHA_A,
                &[],
                &[("example.kernel.uapi", ContractRange::versions(vec![7]))],
            ),
            component("kernel", SHA_B, &[("example.kernel.uapi", 7)], &[]),
        ],
    };
    assert_eq!(
        lock_a.digest(digest_config()).unwrap(),
        lock_b.digest(digest_config()).unwrap()
    );
}

#[test]
fn patch_order_is_semantic_in_lock_identity() {
    let mut first = component("kernel", SHA_A, &[("example.kernel.uapi", 7)], &[]);
    first.patches = vec![DIGEST_1.to_owned(), DIGEST_2.to_owned()];
    let mut second = first.clone();
    second.patches = vec![DIGEST_2.to_owned(), DIGEST_1.to_owned()];

    let lock_a = DependencyLock {
        schema: 1,
        components: vec![first],
    };
    let lock_b = DependencyLock {
        schema: 1,
        components: vec![second],
    };
    assert_ne!(
        lock_a.sha256_digest().unwrap(),
        lock_b.sha256_digest().unwrap()
    );
}

#[test]
fn verify_covers_exact_match_compatible_range_and_every_conflict() {
    let root_manifest = ComponentManifest::from_toml_str(
        "schema = 1\nname = 'app'\n[provides]\n'example.app' = 1\n[requires]\n'example.kernel.uapi' = { min = 7, max = 8 }\n",
    )
    .unwrap();
    let root_digest = root_manifest.sha256_digest().unwrap();
    let set = DependencySet::from_manifest(root_manifest).unwrap();

    let mut app = component("app", SHA_A, &[("example.app", 1)], &[]);
    app.manifest = root_digest;
    let runtime = component(
        "runtime",
        SHA_C,
        &[("example.runtime", 3)],
        &[("example.kernel.uapi", ContractRange::exact(7))],
    );
    let kernel = component("kernel", SHA_B, &[("example.kernel.uapi", 7)], &[]);

    // Compatible intersection: [7..=8] ∩ [7..=7] = {7}, satisfied by 7.
    let ok_lock = DependencyLock {
        schema: 1,
        components: vec![app.clone(), runtime.clone(), kernel.clone()],
    };
    let verified = verify(&set, &ok_lock).unwrap();
    assert_eq!(
        verified.contracts["example.kernel.uapi"].required_range,
        "==7"
    );
    assert_eq!(verified.contracts["example.kernel.uapi"].version, 7);

    // Incompatible version: kernel provides 8 outside {7}.
    let bad_version = DependencyLock {
        schema: 1,
        components: vec![
            app.clone(),
            runtime.clone(),
            component("kernel", SHA_B, &[("example.kernel.uapi", 8)], &[]),
        ],
    };
    assert!(matches!(
        verify(&set, &bad_version),
        Err(DependencySetError::IncompatibleVersion { .. })
    ));

    // Empty intersection: app requires 7..=8, runtime requires 10..=10.
    let empty_intersection = DependencyLock {
        schema: 1,
        components: vec![
            app.clone(),
            component(
                "runtime",
                SHA_C,
                &[],
                &[("example.kernel.uapi", ContractRange::exact(10))],
            ),
            kernel.clone(),
        ],
    };
    assert!(matches!(
        verify(&set, &empty_intersection),
        Err(DependencySetError::EmptyRangeIntersection { .. })
    ));

    // Missing provider.
    let missing_provider = DependencyLock {
        schema: 1,
        components: vec![app.clone(), runtime],
    };
    assert!(matches!(
        verify(&set, &missing_provider),
        Err(DependencySetError::MissingProvider { .. })
    ));

    // Duplicate provider.
    let duplicate_provider = DependencyLock {
        schema: 1,
        components: vec![
            app.clone(),
            kernel.clone(),
            component("kernel_alt", SHA_C, &[("example.kernel.uapi", 7)], &[]),
        ],
    };
    assert!(matches!(
        verify(&set, &duplicate_provider),
        Err(DependencySetError::DuplicateProvider { .. })
    ));

    // Stale manifest digest.
    let mut stale_app = app;
    stale_app.manifest = DIGEST_4.to_owned();
    let stale_lock = DependencyLock {
        schema: 1,
        components: vec![stale_app, kernel],
    };
    assert!(matches!(
        verify(&set, &stale_lock),
        Err(DependencySetError::StaleManifest { .. })
    ));
}

#[test]
fn unrelated_component_change_does_not_invalidate_closure_digest() {
    let lock_before = DependencyLock {
        schema: 1,
        components: vec![
            component("web", SHA_A, &[("example.web", 1)], &[]),
            component("kernel", SHA_B, &[("example.kernel.uapi", 7)], &[]),
            component(
                "host",
                SHA_C,
                &[],
                &[("example.kernel.uapi", ContractRange::exact(7))],
            ),
        ],
    };
    let mut lock_after = lock_before.clone();
    lock_after.component_mut("kernel").unwrap().tree = DIGEST_4.to_owned();

    assert_eq!(
        lock_before.component_closure_digest("web").unwrap(),
        lock_after.component_closure_digest("web").unwrap()
    );
    assert_ne!(
        lock_before.component_closure_digest("host").unwrap(),
        lock_after.component_closure_digest("host").unwrap()
    );
}

#[test]
fn update_selects_newest_compatible_release_and_rejects_non_toml_catalogs() {
    let manifest = ComponentManifest::from_toml_str(
        "schema = 1\nname = 'app'\n[requires]\n'example.kernel.uapi' = { min = 7, max = 7 }\n",
    )
    .unwrap();
    let set = DependencySet::from_manifest(manifest).unwrap();
    let lock = DependencyLock {
        schema: 1,
        components: vec![component(
            "kernel",
            SHA_A,
            &[("example.kernel.uapi", 7)],
            &[],
        )],
    };

    // Catalogs are strictly TOML; JSON input is rejected without sniffing.
    let dir = tempfile::tempdir().unwrap();
    let json_catalog = dir.path().join("catalog.json");
    std::fs::write(&json_catalog, r#"{"schema":1,"candidates":[]}"#).unwrap();
    assert!(matches!(
        CandidateCatalog::from_path(&json_catalog, None),
        Err(DependencySetError::InvalidCatalog(_))
    ));

    let entry = |release, revision: &str, tree: &str, version| CandidateEntry {
        component: "kernel".to_owned(),
        release,
        repository: "https://example.com/kernel".to_owned(),
        revision: revision.to_owned(),
        tree: tree.to_owned(),
        manifest: DIGEST_2.to_owned(),
        patches: vec![],
        provides: BTreeMap::from([("example.kernel.uapi".to_owned(), version)]),
        requires: BTreeMap::new(),
        artifacts: vec![],
        lockfiles: vec![],
    };
    let catalog = CandidateCatalog {
        schema: 1,
        candidates: vec![entry(12, SHA_C, DIGEST_4, 8), entry(11, SHA_B, DIGEST_3, 7)],
    };

    let outcome = update(&set, &lock, &["kernel".to_owned()], &catalog).unwrap();
    assert_eq!(outcome.lock.component("kernel").unwrap().revision, SHA_B);
    assert_eq!(outcome.reports[0].selected_release, 11);
    assert_eq!(
        outcome.reports[0].decisions[0].deciding_contract.as_deref(),
        Some("example.kernel.uapi")
    );
    assert!(!outcome.reports[0].decisions[0].selected);
    assert!(outcome.reports[0].decisions[1].selected);
}

#[test]
fn receipt_rejects_uncommitted_overlay_and_verifies_clean_lock() {
    let manifest = ComponentManifest::from_toml_str(
        "schema = 1\nname = 'app'\n[requires]\n'example.kernel.uapi' = { min = 7, max = 7 }\n",
    )
    .unwrap();
    let set = DependencySet::from_manifest(manifest).unwrap();
    let mut kernel = component("kernel", SHA_B, &[("example.kernel.uapi", 7)], &[]);
    kernel.overlay = Some("../local-kernel".to_owned());

    let overlay_lock = DependencyLock {
        schema: 1,
        components: vec![kernel.clone()],
    };
    let builder = BuilderProvenance {
        identity: "ci-runner".to_owned(),
        workflow: "build".to_owned(),
        run_attempt: 1,
        source_commit: SHA_A.to_owned(),
        build_url: None,
    };
    let artifact = ReceiptArtifact {
        target: "api".to_owned(),
        digest: DIGEST_3.to_owned(),
        image: Some(format!("registry.example.com/app@{DIGEST_3}")),
        task_definition: None,
        action_digest: Some(DIGEST_4.to_owned()),
    };

    assert!(matches!(
        ReleaseReceipt::create(
            &set,
            &overlay_lock,
            vec![artifact.clone()],
            vec![],
            builder.clone(),
            vec![],
        ),
        Err(DependencySetError::OverlayNotCertifiable { .. })
    ));

    kernel.overlay = None;
    let clean_lock = DependencyLock {
        schema: 1,
        components: vec![kernel],
    };
    let receipt = ReleaseReceipt::create(
        &set,
        &clean_lock,
        vec![artifact],
        vec![ActionEvidence {
            name: "unit-tests".to_owned(),
            digest: DIGEST_1.to_owned(),
            outcome: ActionOutcome::Passed,
        }],
        builder,
        vec![],
    )
    .unwrap();
    receipt
        .verify(
            Some(&set.sha256_digest().unwrap()),
            Some(&clean_lock.sha256_digest().unwrap()),
        )
        .unwrap();
}

#[test]
fn native_lockfile_binding_verifies_disk_bytes_and_isolates_ecosystems() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(root.join("Cargo.lock"), "version = 4\n").unwrap();
    std::fs::write(root.join("pnpm-lock.yaml"), "lockfileVersion: '9.0'\n").unwrap();

    let cargo_digest = sha256_prefixed(b"version = 4\n");
    let pnpm_digest = sha256_prefixed(b"lockfileVersion: '9.0'\n");

    let mut rust_comp = component("rust-svc", SHA_A, &[("example.rust", 1)], &[]);
    rust_comp.lockfiles.push(LockedLockfile {
        ecosystem: LockfileEcosystem::Cargo,
        path: "Cargo.lock".to_owned(),
        digest: cargo_digest,
    });
    let mut web_comp = component("web-app", SHA_B, &[("example.web", 1)], &[]);
    web_comp.lockfiles.push(LockedLockfile {
        ecosystem: LockfileEcosystem::Pnpm,
        path: "pnpm-lock.yaml".to_owned(),
        digest: pnpm_digest,
    });

    let set = DependencySet::default();
    let lock = DependencyLock {
        schema: 1,
        components: vec![rust_comp, web_comp],
    };

    let before = verify_in_root(&set, &lock, Some(root)).unwrap();

    // Mutating Cargo.lock fails offline verification before build analysis.
    std::fs::write(root.join("Cargo.lock"), "version = 4\n# updated\n").unwrap();
    assert!(matches!(
        verify_in_root(&set, &lock, Some(root)),
        Err(DependencySetError::StaleLockfile { .. })
    ));

    // Traversal paths fail with InvalidLockfilePath.
    let mut bad_lock = lock.clone();
    bad_lock.components[0].lockfiles[0].path = "../Cargo.lock".to_owned();
    assert!(matches!(
        verify_in_root(&set, &bad_lock, Some(root)),
        Err(DependencySetError::InvalidLockfilePath { .. })
    ));

    // Malformed Cargo.lock fails typed TOML parsing during refresh.
    std::fs::write(root.join("Cargo.lock"), "not valid toml = [\n").unwrap();
    assert!(matches!(
        refresh_lockfiles(&set, &lock, root, &[]),
        Err(DependencySetError::InvalidLock(_))
    ));
    std::fs::write(root.join("Cargo.lock"), "version = 4\n# updated\n").unwrap();

    // Refreshing native lockfiles updates only rust-svc's closure, leaving web-app unchanged.
    let (refreshed_lock, after, count) = refresh_lockfiles(&set, &lock, root, &[]).unwrap();
    assert_eq!(count, 1);
    assert_ne!(
        before.component_closures["rust-svc"],
        after.component_closures["rust-svc"]
    );
    assert_eq!(
        before.component_closures["web-app"],
        after.component_closures["web-app"]
    );
    assert!(verify_in_root(&set, &refreshed_lock, Some(root)).is_ok());
}
