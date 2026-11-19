//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Captures successful file artifacts without changing compiler or cache inputs.

use std::collections::BTreeMap;
use std::io::Write;

use bsmr_artifact::artifact::artifact_dump::ArtifactInfo;
use bsmr_artifact::artifact::artifact_dump::FileInfo;
use bsmr_common::cas_digest::CasDigest;
use bsmr_common::cas_digest::DigestAlgorithm;
use bsmr_common::cas_digest::RawDigest;
use bsmr_common::file_ops::metadata::FileDigestKind;
use bsmr_core::fs::project::ProjectRoot;
use bsmr_core::fs::project_rel_path::ProjectRelativePath;
use serde::Deserialize;
use serde::Serialize;

use super::BuildOutcome;
use super::BuildReport;
use super::EntryLabel;

/// One local component declaration. External dependency selection is not implemented.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct ComponentManifest {
    schema: u32,
    name: String,
    #[serde(default)]
    provides: BTreeMap<String, u64>,
    #[serde(default)]
    requires: BTreeMap<String, String>,
}

/// An experimental output snapshot, not a certification or a complete RFC 0004 lock.
#[derive(Serialize)]
struct DependencyLock {
    schema: &'static str,
    digest_algorithm: &'static str,
    dependency_set: String,
    component: ComponentManifest,
    targets: BTreeMap<String, BTreeMap<String, BTreeMap<String, LockedArtifact>>>,
}

/// File identity and an available command digest. Inline copies have no command digest.
#[derive(Serialize)]
struct LockedArtifact {
    #[serde(flatten)]
    file: FileInfo,
    #[serde(skip_serializing_if = "Option::is_none")]
    action: Option<String>,
}

/// Reject declarations whose semantics this prototype cannot fulfill.
fn manifest(text: &str) -> bsmr_error::Result<ComponentManifest> {
    let value: ComponentManifest = toml::from_str(text).map_err(|e| {
        bsmr_error::bsmr_error!(
            bsmr_error::ErrorTag::Input,
            "invalid component manifest: {}",
            e
        )
    })?;
    if value.schema != 1
        || value.name.is_empty()
        || !value
            .name
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-' || c == b'_')
    {
        return Err(bsmr_error::bsmr_error!(
            bsmr_error::ErrorTag::Input,
            "component requires schema 1 and a simple nonempty name"
        ));
    }
    // TODO(resolver, RFC 0004): verify contract ranges and exact external selections.
    if !value.requires.is_empty() {
        return Err(bsmr_error::bsmr_error!(
            bsmr_error::ErrorTag::Input,
            "snapshot does not yet verify external component requirements"
        ));
    }
    Ok(value)
}

/// Convert the collector's typed results, never paths reread from mutable output files.
fn capture(
    report: &BuildReport,
    component: ComponentManifest,
) -> bsmr_error::Result<DependencyLock> {
    if !report.success || report.results.is_empty() {
        return Err(bsmr_error::bsmr_error!(
            bsmr_error::ErrorTag::Input,
            "snapshot requires successful, nonempty build results"
        ));
    }
    let mut rules = b"bsmr.dependency-set.experimental.v0\0".to_vec();
    rules.extend(serde_json::to_vec(&component)?);
    let dependency_set = format!(
        "sha256:{}",
        CasDigest::<FileDigestKind>::from_content_for_algorithm(&rules, DigestAlgorithm::Sha256)
    );
    let mut targets = BTreeMap::new();
    for (label, entry) in &report.results {
        let EntryLabel::Target(label) = label else {
            return Err(bsmr_error::bsmr_error!(
                bsmr_error::ErrorTag::Input,
                "snapshot requires build targets"
            ));
        };
        if !entry.errors.is_empty() || entry.configured.is_empty() {
            return Err(bsmr_error::bsmr_error!(
                bsmr_error::ErrorTag::Input,
                "snapshot contains failed or skipped target {}",
                label
            ));
        }
        let mut configurations = BTreeMap::new();
        for (configuration, result) in &entry.configured {
            if !matches!(result.inner.success, BuildOutcome::SUCCESS)
                || !result.errors.is_empty()
                || result.artifact_info_by_path.is_empty()
            {
                return Err(bsmr_error::bsmr_error!(
                    bsmr_error::ErrorTag::Input,
                    "snapshot requires successful file outputs for {}",
                    label
                ));
            }
            let mut artifacts = BTreeMap::new();
            for (path, info) in &result.artifact_info_by_path {
                let ArtifactInfo::File(file) = info else {
                    return Err(bsmr_error::bsmr_error!(
                        bsmr_error::ErrorTag::Input,
                        "snapshot only supports files: {}",
                        path
                    ));
                };
                if !matches!(file.digest.raw_digest(), RawDigest::Sha256(_)) {
                    return Err(bsmr_error::bsmr_error!(
                        bsmr_error::ErrorTag::Input,
                        "snapshot requires SHA-256 artifacts"
                    ));
                }
                let action = result.artifact_action_digests.get(path).cloned();
                artifacts.insert(
                    path.to_string(),
                    LockedArtifact {
                        file: file.clone(),
                        action,
                    },
                );
            }
            configurations.insert(configuration.to_string(), artifacts);
        }
        targets.insert(label.to_string(), configurations);
    }
    // TODO(builder, RFC 0004): bind reviewed source trees and exact external locks.
    // TODO(cache, #221): prove identical snapshots after remote action-cache restoration.
    // TODO(publisher, RFC 0004): retain artifact closures durably with separate CI receipts.
    Ok(DependencyLock {
        schema: "bsmr.dependency-lock.experimental.v0",
        digest_algorithm: "sha256",
        dependency_set,
        component,
        targets,
    })
}

/// Publish complete bytes once. Existing snapshots, including older successes, are never replaced.
pub(super) fn write(
    report: &BuildReport,
    root: &ProjectRoot,
    cwd: &ProjectRelativePath,
    filename: &str,
) -> bsmr_error::Result<()> {
    let directory = root.resolve(cwd);
    let declaration = std::fs::read_to_string(directory.as_path().join("bsmr.component.toml"))
        .map_err(|e| {
            bsmr_error::bsmr_error!(
                bsmr_error::ErrorTag::Input,
                "read bsmr.component.toml: {}",
                e
            )
        })?;
    let lock = capture(report, manifest(&declaration)?)?;
    let bytes = toml::to_string_pretty(&lock)
        .map_err(|e| {
            bsmr_error::bsmr_error!(bsmr_error::ErrorTag::Input, "encode snapshot: {}", e)
        })?
        .into_bytes();
    let path = directory.as_path().join(filename);
    let parent = path.parent().ok_or_else(|| {
        bsmr_error::bsmr_error!(
            bsmr_error::ErrorTag::Input,
            "snapshot needs a parent directory"
        )
    })?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|e| {
        bsmr_error::bsmr_error!(bsmr_error::ErrorTag::Input, "create snapshot: {}", e)
    })?;
    temporary.write_all(&bytes).map_err(|e| {
        bsmr_error::bsmr_error!(bsmr_error::ErrorTag::Input, "write snapshot: {}", e)
    })?;
    temporary.as_file().sync_all().map_err(|e| {
        bsmr_error::bsmr_error!(bsmr_error::ErrorTag::Input, "sync snapshot: {}", e)
    })?;
    temporary.persist_noclobber(&path).map_err(|e| {
        bsmr_error::bsmr_error!(
            bsmr_error::ErrorTag::Input,
            "retain snapshot {}: {}",
            path.display(),
            e
        )
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unimplemented_contracts_and_unknown_fields() {
        assert!(manifest("schema=2\nname='hello'").is_err());
        assert!(manifest("schema=1\nname='hello'\nunknown=true").is_err());
        assert!(manifest("schema=1\nname='hello'\n[requires]\nhttp='>=1,<2'").is_err());
    }

    #[test]
    fn manifest_identity_ignores_toml_formatting() {
        let a = manifest("schema=1\nname='hello'\n[provides]\nb=2\na=1").unwrap();
        let b =
            manifest("# same declaration\nname='hello'\nschema=1\n[provides]\na=1\nb=2").unwrap();
        assert_eq!(
            serde_json::to_vec(&a).unwrap(),
            serde_json::to_vec(&b).unwrap()
        );
    }
}
