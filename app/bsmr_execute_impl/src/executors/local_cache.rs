//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Restores and publishes local command results through the shared disk AC/CAS.

use std::collections::BTreeSet;
use std::ops::ControlFlow;
use std::sync::Arc;

use async_trait::async_trait;
use bsmr_common::file_ops::metadata::FileMetadata;
use bsmr_common::file_ops::metadata::TrackedFileDigest;
use bsmr_core::fs::artifact_path_resolver::ArtifactFs;
use bsmr_directory::directory::directory::Directory;
use bsmr_directory::directory::directory_iterator::DirectoryIterator;
use bsmr_directory::directory::directory_iterator::DirectoryIteratorPathStack;
use bsmr_directory::directory::entry::DirectoryEntry;
use bsmr_directory::directory::walk::unordered_entry_walk;
use bsmr_error::BsmrErrorContext;
use bsmr_execute::artifact_value::ArtifactValue;
use bsmr_execute::digest_config::DigestConfig;
use bsmr_execute::directory::ActionDirectoryMember;
use bsmr_execute::directory::directory_to_re_tree;
use bsmr_execute::directory::extract_artifact_value;
use bsmr_execute::directory::re_tree_to_directory;
use bsmr_execute::execute::blocking::BlockingExecutor;
use bsmr_execute::execute::cache_uploader::CacheUploadInfo;
use bsmr_execute::execute::cache_uploader::CacheUploadOutcome;
use bsmr_execute::execute::cache_uploader::CacheUploadResults;
use bsmr_execute::execute::cache_uploader::DepFileCacheUploadOutcome;
use bsmr_execute::execute::cache_uploader::IntoRemoteDepFile;
use bsmr_execute::execute::cache_uploader::UploadCache;
use bsmr_execute::execute::executor_stage_async;
use bsmr_execute::execute::kind::CommandExecutionKind;
use bsmr_execute::execute::local_cache::LocalActionCache;
use bsmr_execute::execute::local_cache::LocalActionResult;
use bsmr_execute::execute::local_cache::LocalDigest;
use bsmr_execute::execute::local_cache::LocalOutputDirectory;
use bsmr_execute::execute::local_cache::LocalOutputFile;
use bsmr_execute::execute::local_cache::parallel_cache_io;
use bsmr_execute::execute::manager::CommandExecutionManager;
use bsmr_execute::execute::manager::CommandExecutionManagerExt;
use bsmr_execute::execute::output::CommandStdStreams;
use bsmr_execute::execute::prepared::PreparedCommand;
use bsmr_execute::execute::prepared::PreparedCommandOptionalExecutor;
use bsmr_execute::execute::request::CommandExecutionOutput;
use bsmr_execute::execute::result::CommandExecutionMetadata;
use bsmr_execute::execute::result::CommandExecutionResult;
use bsmr_execute::materialize::materializer::DeclareArtifactPayload;
use bsmr_execute::materialize::materializer::Materializer;
use bsmr_fs::paths::forward_rel_path::ForwardRelativePath;
use bsmr_hash::BsmrIndexMap;
use bsmr_util::time_span::TimeSpan;
use chrono::DateTime;
use chrono::Utc;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use prost::Message;
use remote_execution::TActionResult2;

use crate::incremental_actions_helper::save_content_based_incremental_state;

/// Checks the user-level disk cache before invoking a local command.
pub struct LocalActionCacheChecker {
    pub artifact_fs: ArtifactFs,
    pub materializer: Arc<dyn Materializer>,
    pub incremental_db_state: Arc<crate::sqlite::incremental_state_db::IncrementalDbState>,
    pub blocking_executor: Arc<dyn BlockingExecutor>,
    pub cache: Arc<LocalActionCache>,
}

#[async_trait]
impl PreparedCommandOptionalExecutor for LocalActionCacheChecker {
    async fn maybe_execute(
        &self,
        command: &PreparedCommand<'_, '_>,
        manager: CommandExecutionManager,
        _cancellations: &CancellationContext,
    ) -> ControlFlow<CommandExecutionResult, CommandExecutionManager> {
        let action = command.prepared_action.action_and_blobs.action.dupe();
        let result = match executor_stage_async(
            bsmr_data::CacheQuery {
                action_digest: action.to_string(),
                cache_type: bsmr_data::CacheType::ActionCache.into(),
            },
            self.blocking_executor
                .execute_io_inline(|| self.cache.action_result(&action)),
        )
        .await
        {
            Ok(Some(result)) => result,
            Ok(None) => return ControlFlow::Continue(manager),
            Err(error) => return ControlFlow::Break(manager.error("local_action_cache", error)),
        };

        let start = TimeSpan::start_now();
        let manager = manager.claim().await;
        let restored = executor_stage_async(
            bsmr_data::CacheHit {
                action_digest: action.to_string(),
                action_key: None,
                cache_type: bsmr_data::CacheType::ActionCache.into(),
            },
            restore_result(
                &self.artifact_fs,
                self.materializer.as_ref(),
                self.cache.dupe(),
                command,
                result,
            ),
        )
        .await;
        let (outputs, streams) = match restored {
            Ok(restored) => restored,
            Err(error) => {
                return ControlFlow::Break(manager.error("local_action_cache_restore", error));
            }
        };

        let mut result = manager.success(
            CommandExecutionKind::LocalActionCache {
                digest: action.dupe(),
            },
            outputs,
            streams,
            CommandExecutionMetadata::empty(start.end_now()),
        );
        if let Some(run_action_key) = command.request.run_action_key()
            && !command.request.outputs_cleanup
        {
            save_content_based_incremental_state(
                run_action_key.clone(),
                &self.incremental_db_state,
                &self.artifact_fs,
                &result,
            );
        }
        tracing::info!(action = %action, "restored action from the local disk cache");
        result.cache_upload_result = bsmr_data::UploadResult::NotAttempted;
        ControlFlow::Break(result)
    }
}

/// Publishes successful local command results to the user-level disk cache.
pub struct LocalActionCacheUploader {
    pub artifact_fs: ArtifactFs,
    pub blocking_executor: Arc<dyn BlockingExecutor>,
    pub cache: Arc<LocalActionCache>,
}

#[async_trait]
impl UploadCache for LocalActionCacheUploader {
    fn is_local_action_cache(&self) -> bool {
        true
    }

    async fn upload(
        &self,
        info: &CacheUploadInfo<'_>,
        result: &CommandExecutionResult,
        _re_result: Option<TActionResult2>,
        _dep_file_bundle: Option<&mut dyn IntoRemoteDepFile>,
        action: &bsmr_execute::execute::action_digest_and_blobs::ActionDigestAndBlobs,
    ) -> bsmr_error::Result<CacheUploadResults> {
        if !result.was_locally_executed() {
            return Ok(CacheUploadResults {
                cache_upload_outcome: CacheUploadOutcome::NonLocalExecution,
                dep_file_cache_upload_outcome: DepFileCacheUploadOutcome::NoDepFileBundle,
                dep_file_cache_upload_key: None,
            });
        }
        if !has_supported_output_roots(&self.artifact_fs, result)? {
            tracing::info!(
                action = %action.action,
                "kept an unsupported symlink output root out of the local action cache"
            );
            return Ok(CacheUploadResults {
                cache_upload_outcome: CacheUploadOutcome::ExecutorUploadDisabled,
                dep_file_cache_upload_outcome: DepFileCacheUploadOutcome::NoDepFileBundle,
                dep_file_cache_upload_key: None,
            });
        }
        let action_name = info.target.as_proto_action_name();
        if action_name.category == "python_wheel"
            && !is_portable_python_wheel_result(&self.artifact_fs, result)?
        {
            tracing::info!(
                action = %action.action,
                "kept a platform wheel out of the local action cache"
            );
            return Ok(CacheUploadResults {
                cache_upload_outcome: CacheUploadOutcome::ExecutorUploadDisabled,
                dep_file_cache_upload_outcome: DepFileCacheUploadOutcome::NoDepFileBundle,
                dep_file_cache_upload_key: None,
            });
        }
        let streams = result.report.std_streams.clone().into_bytes().await?;
        self.blocking_executor
            .execute_io_inline(|| {
                publish_result(
                    &self.artifact_fs,
                    &self.cache,
                    info.digest_config,
                    result,
                    &streams.stdout,
                    &streams.stderr,
                    &action.action,
                )
            })
            .await?;
        tracing::debug!(action = %action.action, "published action to the local disk cache");
        Ok(CacheUploadResults {
            cache_upload_outcome: CacheUploadOutcome::Success,
            dep_file_cache_upload_outcome: DepFileCacheUploadOutcome::NoDepFileBundle,
            dep_file_cache_upload_key: None,
        })
    }
}

async fn restore_result(
    artifact_fs: &ArtifactFs,
    materializer: &dyn Materializer,
    cache: Arc<LocalActionCache>,
    command: &PreparedCommand<'_, '_>,
    result: LocalActionResult,
) -> bsmr_error::Result<(
    BsmrIndexMap<CommandExecutionOutput, ArtifactValue>,
    CommandStdStreams,
)> {
    let digest_config = command.digest_config;
    validate_output_paths(
        command
            .request
            .paths()
            .output_paths()
            .iter()
            .map(|(path, _)| path.to_string()),
        &result,
    )?;
    let mut input = command
        .request
        .paths()
        .input_directory()
        .clone()
        .into_builder();
    for file in &result.output_files {
        let digest = file.digest.to_file_digest(digest_config)?;
        input.insert(
            re_path(&file.path)?,
            DirectoryEntry::Leaf(ActionDirectoryMember::File(FileMetadata {
                digest,
                is_executable: file.executable,
            })),
        )?;
    }
    for output_directory in &result.output_directories {
        let digest = &output_directory.tree_digest;
        digest.to_file_digest(digest_config)?;
        let bytes = cache.read_blob(digest, digest_config)?.ok_or_else(|| {
            bsmr_error::bsmr_error!(
                bsmr_error::ErrorTag::MaterializationError,
                "Local cache tree disappeared after lookup: {}",
                digest.hash
            )
        })?;
        let tree = remote_execution::Tree::decode(bytes.as_slice())?;
        let directory =
            re_tree_to_directory(&tree, &DateTime::<Utc>::MAX_UTC, digest_config, true)?;
        input.insert(
            re_path(&output_directory.path)?,
            DirectoryEntry::Dir(directory),
        )?;
    }

    let mut declarations = Vec::with_capacity(command.request.paths().output_paths().len());
    let mut outputs = BsmrIndexMap::with_capacity(command.request.paths().output_paths().len());
    for (requested, (path, _)) in command
        .request
        .outputs()
        .zip(command.request.paths().output_paths())
    {
        if let Some(value) = extract_artifact_value(&input, path, digest_config)? {
            let configuration_path = if materializer.is_eager_materialization_enabled()
                && requested.has_content_based_path()
            {
                Some(
                    requested
                        .resolve_configuration_hash_path(artifact_fs)?
                        .path
                        .to_owned(),
                )
            } else {
                None
            };
            declarations.push(DeclareArtifactPayload {
                path: requested
                    .resolve(
                        artifact_fs,
                        requested
                            .has_content_based_path()
                            .then(|| value.content_based_path_hash())
                            .as_ref(),
                    )?
                    .path
                    .to_owned(),
                artifact: value.dupe(),
                configuration_path,
            });
            outputs.insert(requested.cloned(), value);
        }
    }

    materializer
        .declare_local_cache_many(cache.dupe(), digest_config, declarations)
        .await?;

    let stdout = read_stream(&cache, &result.stdout, digest_config)?;
    let stderr = read_stream(&cache, &result.stderr, digest_config)?;
    Ok((outputs, CommandStdStreams::Local { stdout, stderr }))
}

fn validate_output_paths(
    expected: impl IntoIterator<Item = String>,
    result: &LocalActionResult,
) -> bsmr_error::Result<()> {
    let expected = expected.into_iter().collect::<BTreeSet<_>>();
    let paths = result
        .output_files
        .iter()
        .map(|output| output.path.as_str())
        .chain(
            result
                .output_directories
                .iter()
                .map(|output| output.path.as_str()),
        )
        .collect::<Vec<_>>();
    let actual = paths
        .iter()
        .map(|path| {
            let normalized = re_path(path)?;
            if normalized.as_str() != *path {
                return Err(bsmr_error::bsmr_error!(
                    bsmr_error::ErrorTag::MaterializationError,
                    "Local action cache output path is not canonical: {path}"
                ));
            }
            Ok(path.to_string())
        })
        .collect::<bsmr_error::Result<BTreeSet<_>>>()?;
    if paths.len() != actual.len() || actual != expected {
        return Err(bsmr_error::bsmr_error!(
            bsmr_error::ErrorTag::MaterializationError,
            "Local action cache output set does not match declared outputs: expected {expected:?}, cached {actual:?}"
        ));
    }
    Ok(())
}

fn read_stream(
    cache: &LocalActionCache,
    digest: &Option<LocalDigest>,
    digest_config: DigestConfig,
) -> bsmr_error::Result<Vec<u8>> {
    let Some(digest) = digest else {
        return Ok(Vec::new());
    };
    cache.read_blob(digest, digest_config)?.ok_or_else(|| {
        bsmr_error::bsmr_error!(
            bsmr_error::ErrorTag::MaterializationError,
            "Local cache stream disappeared after lookup: {}:{}",
            digest.hash,
            digest.size
        )
    })
}

fn publish_result(
    artifact_fs: &ArtifactFs,
    cache: &LocalActionCache,
    digest_config: DigestConfig,
    result: &CommandExecutionResult,
    stdout: &[u8],
    stderr: &[u8],
    action: &bsmr_execute::execute::action_digest::ActionDigest,
) -> bsmr_error::Result<()> {
    let mut manifest = LocalActionResult::default();
    let mut files = Vec::new();
    let mut trees = Vec::new();
    for output in result.resolve_outputs(artifact_fs) {
        let (output, value) = output?;
        match value.entry().as_ref() {
            DirectoryEntry::Leaf(ActionDirectoryMember::File(file)) => {
                files.push((
                    file.digest.dupe(),
                    artifact_fs.fs().resolve(output.path()).into_path_buf(),
                ));
                manifest.output_files.push(LocalOutputFile {
                    path: output.path().to_string(),
                    digest: LocalDigest::from_file(&file.digest),
                    executable: file.is_executable,
                });
            }
            DirectoryEntry::Dir(directory) => {
                let root = artifact_fs.fs().resolve(output.path());
                let mut walk =
                    unordered_entry_walk(DirectoryEntry::Dir(directory).map_dir(Directory::as_ref));
                while let Some((relative, entry)) = walk.next() {
                    if let DirectoryEntry::Leaf(ActionDirectoryMember::File(file)) = entry {
                        files.push((
                            file.digest.dupe(),
                            root.join(relative.get()).into_path_buf(),
                        ));
                    }
                }
                let tree = directory_to_re_tree(directory);
                let bytes = tree.encode_to_vec();
                let tree_digest =
                    TrackedFileDigest::from_content(&bytes, digest_config.cas_digest_config());
                trees.push((tree_digest.dupe(), bytes));
                manifest.output_directories.push(LocalOutputDirectory {
                    path: output.path().to_string(),
                    tree_digest: LocalDigest::from_file(&tree_digest),
                });
            }
            DirectoryEntry::Leaf(
                ActionDirectoryMember::Symlink(_) | ActionDirectoryMember::ExternalSymlink(_),
            ) => {
                return Err(bsmr_error::bsmr_error!(
                    bsmr_error::ErrorTag::CacheUploadFailed,
                    "Local action cache does not support a symlink as an output root: {}",
                    output.path()
                ));
            }
        }
    }
    parallel_cache_io(&files, |(digest, source)| {
        cache.publish_file(digest, source, digest_config)
    })?;
    for (digest, bytes) in trees {
        cache.publish_bytes(&digest, &bytes, digest_config)?;
    }
    manifest.stdout = publish_stream(cache, stdout, digest_config)?;
    manifest.stderr = publish_stream(cache, stderr, digest_config)?;
    cache.publish_action_result(action, &manifest)
}

fn publish_stream(
    cache: &LocalActionCache,
    bytes: &[u8],
    digest_config: DigestConfig,
) -> bsmr_error::Result<Option<LocalDigest>> {
    if bytes.is_empty() {
        return Ok(None);
    }
    let digest = TrackedFileDigest::from_content(bytes, digest_config.cas_digest_config());
    cache.publish_bytes(&digest, bytes, digest_config)?;
    Ok(Some(LocalDigest::from_file(&digest)))
}

fn re_path(path: &str) -> bsmr_error::Result<&ForwardRelativePath> {
    ForwardRelativePath::new_trim_trailing_slashes(path)
        .bsmr_error_context("Path received from the local cache is not normalized")
}

fn is_portable_python_wheel_result(
    artifact_fs: &ArtifactFs,
    result: &CommandExecutionResult,
) -> bsmr_error::Result<bool> {
    let mut wheel_count = 0;
    let mut all_portable = true;
    for output in result.resolve_outputs(artifact_fs) {
        let (_, value) = output?;
        let DirectoryEntry::Dir(directory) = value.entry().as_ref() else {
            continue;
        };
        let mut walk = unordered_entry_walk(DirectoryEntry::Dir(directory.as_ref()));
        while let Some((path, entry)) = walk.next() {
            if !matches!(entry, DirectoryEntry::Leaf(ActionDirectoryMember::File(_))) {
                continue;
            }
            let Some(name) = path.path().last().map(|name| name.as_str()) else {
                continue;
            };
            if name.ends_with(".whl") {
                wheel_count += 1;
                all_portable &= pure_python_wheel_name(name);
            }
        }
    }
    Ok(wheel_count == 1 && all_portable)
}

fn has_supported_output_roots(
    artifact_fs: &ArtifactFs,
    result: &CommandExecutionResult,
) -> bsmr_error::Result<bool> {
    for output in result.resolve_outputs(artifact_fs) {
        let (_, value) = output?;
        if matches!(
            value.entry().as_ref(),
            DirectoryEntry::Leaf(
                ActionDirectoryMember::Symlink(_) | ActionDirectoryMember::ExternalSymlink(_)
            )
        ) {
            return Ok(false);
        }
    }
    Ok(true)
}

fn pure_python_wheel_name(name: &str) -> bool {
    let Some(stem) = name.strip_suffix(".whl") else {
        return false;
    };
    let mut fields = stem.rsplitn(4, '-');
    matches!(fields.next(), Some("any"))
        && matches!(fields.next(), Some("none"))
        && fields.next().is_some_and(|python| python.starts_with("py"))
        && fields.next().is_some()
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    use bsmr_execute::execute::local_cache::LocalActionResult;
    use bsmr_execute::execute::local_cache::LocalDigest;
    use bsmr_execute::execute::local_cache::LocalOutputFile;

    use super::parallel_cache_io;
    use super::pure_python_wheel_name;
    use super::validate_output_paths;

    #[test]
    fn only_portable_pure_python_wheels_are_locally_cacheable() {
        assert!(pure_python_wheel_name(
            "django-6.2.dev20260813060032-py3-none-any.whl"
        ));
        assert!(pure_python_wheel_name("demo-1.0-py2.py3-none-any.whl"));
        assert!(!pure_python_wheel_name(
            "pyroaring-1.0-cp314-cp314-macosx_11_0_arm64.whl"
        ));
        assert!(!pure_python_wheel_name("not-a-wheel.txt"));
    }

    #[test]
    fn parallel_io_visits_every_value_exactly_once() -> bsmr_error::Result<()> {
        let values = (0..257).collect::<Vec<_>>();
        let visits = values
            .iter()
            .map(|_| AtomicUsize::new(0))
            .collect::<Vec<_>>();

        parallel_cache_io(&values, |value| {
            visits[*value].fetch_add(1, Ordering::Relaxed);
            Ok(())
        })?;

        assert!(
            visits
                .iter()
                .all(|visits| visits.load(Ordering::Relaxed) == 1)
        );
        Ok(())
    }

    #[test]
    fn cached_outputs_must_exactly_match_declared_outputs() -> bsmr_error::Result<()> {
        let result = LocalActionResult {
            output_files: vec![LocalOutputFile {
                path: "bsmr-out/result".to_owned(),
                digest: LocalDigest {
                    algorithm: "SHA256".to_owned(),
                    hash: "00".repeat(32),
                    size: 0,
                },
                executable: false,
            }],
            ..Default::default()
        };

        validate_output_paths(["bsmr-out/result".to_owned()], &result)?;
        assert!(validate_output_paths(["bsmr-out/other".to_owned()], &result).is_err());
        assert!(validate_output_paths([], &result).is_err());
        Ok(())
    }
}
