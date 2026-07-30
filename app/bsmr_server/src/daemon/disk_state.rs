/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use std::sync::Arc;

use allocative::Allocative;
use bsmr_common::invocation_paths::InvocationPaths;
use bsmr_common::legacy_configs::configs::LegacyBuckConfig;
use bsmr_common::legacy_configs::key::BuckconfigKeyRef;
use bsmr_core::rollout_percentage::RolloutPercentage;
use bsmr_error::BuckErrorContext;
use bsmr_error::internal_error;
use bsmr_events::daemon_id::DaemonId;
use bsmr_execute::digest_config::DigestConfig;
use bsmr_execute::execute::blocking::BlockingExecutor;
use bsmr_execute::materialize::materializer::MaterializationMethod;
use bsmr_execute_impl::materializers::deferred::DeferredMaterializerConfigs;
use bsmr_execute_impl::sqlite::incremental_state_db::INCREMENTAL_DB_SCHEMA_VERSION;
use bsmr_execute_impl::sqlite::incremental_state_db::IncrementalDbState;
use bsmr_execute_impl::sqlite::incremental_state_db::IncrementalStateSqliteDb;
use bsmr_execute_impl::sqlite::materializer_db::MATERIALIZER_DB_SCHEMA_VERSION;
use bsmr_execute_impl::sqlite::materializer_db::MaterializerState;
use bsmr_execute_impl::sqlite::materializer_db::MaterializerStateSqliteDb;
use bsmr_fs::error::IoResultExt;
use bsmr_fs::fs_util;
use bsmr_fs::paths::abs_norm_path::AbsNormPath;
use bsmr_fs::paths::file_name::FileName;
use bsmr_hash::StdBuckHashMap;

use crate::daemon::server::BuckdServerInitPreferences;

#[derive(Allocative)]
pub struct DiskStateOptions {
    pub sqlite_materializer_state: bool,
    // In future, this will include the config for dep files on disk
}

impl DiskStateOptions {
    pub fn new(
        root_config: &LegacyBuckConfig,
        materialization_method: MaterializationMethod,
    ) -> bsmr_error::Result<Self> {
        let sqlite_materializer_state = matches!(
            // We can only enable materializer state on sqlite if you use deferred materializer
            materialization_method,
            MaterializationMethod::Deferred | MaterializationMethod::DeferredSkipFinalArtifacts
        ) && root_config
            .parse::<RolloutPercentage>(BuckconfigKeyRef {
                section: "bsmr",
                property: "sqlite_materializer_state",
            })?
            .unwrap_or_else(RolloutPercentage::always)
            .roll();
        Ok(Self {
            sqlite_materializer_state,
        })
    }
}

fn sqlite_db_setup_metadata_and_versions(
    root_config: &LegacyBuckConfig,
    schema_version: String,
    version_config: &str,
    deferred_materializer_config: Option<&DeferredMaterializerConfigs>,
    daemon_id: &DaemonId,
) -> bsmr_error::Result<(
    StdBuckHashMap<String, String>,
    StdBuckHashMap<String, String>,
)> {
    let metadata = bsmr_events::metadata::collect(daemon_id);

    let mut versions = StdBuckHashMap::from([("schema_version".to_owned(), schema_version)]);

    if let Some(config) = deferred_materializer_config {
        versions.insert(
            "defer_write_actions".to_owned(),
            config.defer_write_actions.to_string(),
        );
    }

    if let Some(buckconfig_version) = root_config.parse(BuckconfigKeyRef {
        section: "bsmr",
        property: version_config,
    })? {
        versions.insert("buckconfig_version".to_owned(), buckconfig_version);
    }
    if let Some(hostname) = metadata.get("hostname") {
        versions.insert("hostname".to_owned(), hostname.to_owned());
    }

    Ok((metadata, versions))
}

pub(crate) async fn maybe_initialize_materializer_sqlite_db(
    options: &DiskStateOptions,
    paths: InvocationPaths,
    io_executor: Arc<dyn BlockingExecutor>,
    root_config: &LegacyBuckConfig,
    deferred_materializer_configs: &DeferredMaterializerConfigs,
    digest_config: DigestConfig,
    init_ctx: &BuckdServerInitPreferences,
    daemon_id: &DaemonId,
) -> bsmr_error::Result<(Option<MaterializerStateSqliteDb>, Option<MaterializerState>)> {
    if !options.sqlite_materializer_state {
        // When sqlite materializer state is disabled, we should always delete the materializer state db.
        // Otherwise, artifacts in buck-out will diverge from the state stored in db.
        io_executor
            .execute_io_inline(|| {
                fs_util::remove_all(paths.materializer_state_path())
                    .categorize_internal()
                    .map_err(bsmr_error::Error::from)
            })
            .await?;
        return Ok((None, None));
    }

    let (metadata, versions) = sqlite_db_setup_metadata_and_versions(
        root_config,
        MATERIALIZER_DB_SCHEMA_VERSION.to_string(),
        "sqlite_materializer_state_version",
        Some(deferred_materializer_configs),
        daemon_id,
    )?;

    // Most things in the rest of `metadata` should go in the metadata sqlite table.
    // TODO(scottcao): Narrow down what metadata we need and insert them into the
    // metadata table before a feature rollout.
    let (db, load_result) = MaterializerStateSqliteDb::initialize(
        paths.materializer_state_path(),
        versions,
        metadata,
        io_executor,
        digest_config,
        init_ctx.reject_materializer_state.as_ref(),
    )
    .await?;

    // We know path not found or version mismatch is normal, but some sqlite failures
    // are worth logging here. TODO(scottcao): Refine our error types and figure out what
    // errors to log
    let materializer_state = load_result.ok();
    Ok((Some(db), materializer_state))
}

pub(crate) async fn maybe_initialize_incremental_sqlite_db(
    paths: InvocationPaths,
    io_executor: Arc<dyn BlockingExecutor>,
    root_config: &LegacyBuckConfig,
    daemon_id: &DaemonId,
) -> bsmr_error::Result<IncrementalDbState> {
    // Rolling it out by default, but giving an option to disable in case something goes horribly wrong
    if !root_config
        .parse(BuckconfigKeyRef {
            section: "bsmr",
            property: "sqlite_incremental_state",
        })?
        .unwrap_or(true)
    {
        // When sqlite incremental state is disabled, we should always delete the db to
        // prevent futures invocations from potentially using stale entries
        io_executor
            .execute_io_inline(|| {
                fs_util::remove_all(paths.incremental_state_path())
                    .categorize_internal()
                    .map_err(bsmr_error::Error::from)
            })
            .await?;
        return Ok(IncrementalDbState::db_disabled());
    }

    let (metadata, versions) = sqlite_db_setup_metadata_and_versions(
        root_config,
        INCREMENTAL_DB_SCHEMA_VERSION.to_string(),
        "sqlite_incremental_state_version",
        None,
        daemon_id,
    )?;

    let incremental_db_state = IncrementalStateSqliteDb::initialize(
        paths.incremental_state_path(),
        versions,
        metadata,
        io_executor,
        // TODO(minglunli): I'm not convinced we need reject_identity for incremental state. iiuc, this is only used by restarter
        // but incremental state isn't as widely used as materializer so we prob shouldn't restart daemon even if that's out of sync?
        None,
    )
    .await?;
    Ok(incremental_db_state)
}

// Once we start storing disk state in the cache directory, we need to make sure
// bsmr always deletes the cache directory if the cache is disabled.
// Otherwise, buck-out state can diverge from the state of on-disk cache when
// cache is disabled, causing bsmr to use stale cache when reading from the
// cache is re-enabled. One way this can happen is that someone can build on
// an older revision with a bsmr that doesn't understand the cache directory
// in between 2 builds on newer revisions with bsmr that reads from the cache
// (for ex., as a part of a bisect), then the state can become stale.
// There are 2 (not foolproof) mitigations planned:
// 1) Read from the logs what the last bsmr invocation was and check that the
// last bsmr supported on-disk state. If not, delete the disk state.
// 2) Start always deleting the cache directory now until we add support for disk
// state in bsmr.
// The following implements mitigation #2 by always deleting disk state.

/// Recursively deletes all elements under `cache_dir_path`, except for known dirs
/// listed in `known_dir_names`.
pub(crate) fn delete_unknown_disk_state(
    cache_dir_path: &AbsNormPath,
    known_dir_names: &[&FileName],
) -> bsmr_error::Result<()> {
    let res: bsmr_error::Result<()> = try {
        if cache_dir_path.exists() {
            for entry in fs_util::read_dir(cache_dir_path).categorize_internal()? {
                let entry = entry.map_err(bsmr_error::Error::from)?;
                let filename = entry.file_name();
                let filename = filename
                    .to_str()
                    .ok_or_else(|| internal_error!("Filename is not UTF-8"))
                    .and_then(FileName::new)?;

                // known_dir_names is always small, so this contains isn't expensive
                if !known_dir_names.contains(&filename) || !entry.path().is_dir() {
                    fs_util::remove_all(cache_dir_path.join(filename)).categorize_internal()?;
                }
            }
        }
    };

    res.with_buck_error_context(|| {
        format!(
            "deleting unrecognized caches in {} to prevent them from going stale",
            &cache_dir_path
        )
    })
}

#[cfg(test)]
mod tests {
    use bsmr_core::fs::project::ProjectRootTemp;
    use bsmr_core::fs::project_rel_path::ProjectRelativePath;
    use bsmr_fs::fs_util::uncategorized as fs_util;
    use bsmr_fs::paths::forward_rel_path::ForwardRelativePath;

    use super::*;

    #[test]
    fn test_delete_all_from_cache_dir() {
        let fs_temp = ProjectRootTemp::new().unwrap();
        let fs = fs_temp.path();
        let cache_dir_path = fs.resolve(ProjectRelativePath::unchecked_new("buck-out/v2/cache"));
        let materializer_state_db = cache_dir_path.join(ForwardRelativePath::unchecked_new(
            "materializer_state/db.sqlite",
        ));
        let command_hashes_db = cache_dir_path.join(ForwardRelativePath::unchecked_new(
            "command_hashes/db.sqlite",
        ));
        fs_util::create_dir_all(materializer_state_db.parent().unwrap()).unwrap();
        fs_util::write(&materializer_state_db, b"").unwrap();
        fs_util::create_dir_all(command_hashes_db.parent().unwrap()).unwrap();
        fs_util::write(&command_hashes_db, b"").unwrap();
        assert!(materializer_state_db.exists());
        assert!(command_hashes_db.exists());

        delete_unknown_disk_state(&cache_dir_path, &[]).unwrap();

        assert!(!materializer_state_db.exists());
        assert!(!command_hashes_db.exists());
    }

    #[test]
    fn test_delete_from_cache_dir_with_known_dirs() {
        let fs_temp = ProjectRootTemp::new().unwrap();
        let fs = fs_temp.path();
        let cache_dir_path = fs.resolve(ProjectRelativePath::unchecked_new("buck-out/v2/cache"));
        let materializer_state_db = cache_dir_path.join(ForwardRelativePath::unchecked_new(
            "materializer_state/db.sqlite",
        ));
        let command_hashes_db = cache_dir_path.join(ForwardRelativePath::unchecked_new(
            "command_hashes/db.sqlite",
        ));
        fs_util::create_dir_all(materializer_state_db.parent().unwrap()).unwrap();
        fs_util::write(&materializer_state_db, b"").unwrap();
        fs_util::create_dir_all(command_hashes_db.parent().unwrap()).unwrap();
        fs_util::write(&command_hashes_db, b"").unwrap();
        assert!(materializer_state_db.exists());
        assert!(command_hashes_db.exists());

        delete_unknown_disk_state(
            &cache_dir_path,
            &[FileName::unchecked_new("materializer_state")],
        )
        .unwrap();

        assert!(materializer_state_db.exists());
        assert!(!command_hashes_db.exists());
    }
}
