//===----------------------------------------------------------------------===//
// Upstream-Source: facebook/buck2@1560aca2002865cd73d7cafb22c705cfb640b2bc
// Modifications Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

/*
 * Copyright (c) Meta Platforms, Inc. and affiliates.
 *
 * This source code is dual-licensed under either the MIT license found in the
 * LICENSE-MIT file in the root directory of this source tree or the Apache
 * License, Version 2.0 found in the LICENSE-APACHE file in the root directory
 * of this source tree. You may select, at your option, one of the
 * above-listed licenses.
 */

use async_trait::async_trait;
use bsmr_action_metadata_proto::RemoteDepFile;
use bsmr_core::bsmr_env;
use bsmr_core::fs::artifact_path_resolver::ArtifactFs;
use remote_execution::TActionResult2;
use remote_execution::TCode;
use tracing::info;
use tracing::warn;

use crate::digest_config::DigestConfig;
use crate::execute::action_digest_and_blobs::ActionDigestAndBlobs;
use crate::execute::dep_file_digest::DepFileDigest;
use crate::execute::result::CommandExecutionResult;
use crate::execute::target::CommandExecutionTarget;
use crate::materialize::materializer::Materializer;
use crate::re::error::RemoteExecutionError;

pub struct CacheUploadInfo<'a> {
    pub target: &'a dyn CommandExecutionTarget,
    pub digest_config: DigestConfig,
    pub mergebase: &'a Option<String>,
    pub re_platform: &'a remote_execution::Platform,
}

#[async_trait]
pub trait IntoRemoteDepFile: Send {
    fn remote_dep_file_action(
        &self,
        digest_config: DigestConfig,
        mergebase: &Option<String>,
        re_platform: &remote_execution::Platform,
    ) -> ActionDigestAndBlobs;

    async fn make_remote_dep_file(
        &mut self,
        digest_config: DigestConfig,
        fs: &ArtifactFs,
        materializer: &dyn Materializer,
        result: &CommandExecutionResult,
    ) -> bsmr_error::Result<Option<RemoteDepFile>>;
}

pub struct CacheUploadResults {
    pub cache_upload_outcome: CacheUploadOutcome,
    pub dep_file_cache_upload_outcome: DepFileCacheUploadOutcome,
    pub dep_file_cache_upload_key: Option<DepFileDigest>,
}

pub enum DepFileCacheUploadOutcome {
    NoDepFileBundle,
    UnsupportedExecutionKind,
    Attempted(CacheUploadOutcome),
}

impl DepFileCacheUploadOutcome {
    pub fn attempted(outcome: CacheUploadOutcome) -> Self {
        Self::Attempted(outcome)
    }

    pub fn outcome(&self) -> Option<&CacheUploadOutcome> {
        match self {
            Self::NoDepFileBundle | Self::UnsupportedExecutionKind => None,
            Self::Attempted(outcome) => Some(outcome),
        }
    }

    pub fn uploaded(&self) -> bool {
        match self {
            Self::NoDepFileBundle | Self::UnsupportedExecutionKind => false,
            Self::Attempted(outcome) => outcome.uploaded(),
        }
    }

    pub fn to_proto(&self) -> bsmr_data::UploadResult {
        match self {
            Self::NoDepFileBundle | Self::UnsupportedExecutionKind => {
                bsmr_data::UploadResult::NotAttempted
            }
            Self::Attempted(outcome) => outcome.to_proto(),
        }
    }
}

pub enum CacheUploadOutcome {
    Success,
    RejectedOutputExceedsLimit { max_bytes: u64 },
    RejectedPermissionDenied { reason: String },
    RejectedSymlinkOutput,
    FailedUploadActionBlobs { error: bsmr_error::Error },
    FailedUploadOutputs { error: bsmr_error::Error },
    FailedWriteActionResult { error: bsmr_error::Error },
    FailedOther { error: bsmr_error::Error },
    ExecutorUploadDisabled,
    HadDepFileBundle,
    NonLocalExecution,
}

impl CacheUploadOutcome {
    pub fn did_cache_upload(&self) -> bool {
        matches!(self, Self::Success)
    }

    pub fn uploaded(&self) -> bool {
        self.did_cache_upload()
    }

    pub fn to_proto(&self) -> bsmr_data::UploadResult {
        match self {
            Self::Success => bsmr_data::UploadResult::Uploaded,
            Self::RejectedOutputExceedsLimit { .. } => {
                bsmr_data::UploadResult::RejectedOutputExceedsLimit
            }
            Self::RejectedPermissionDenied { .. } => {
                bsmr_data::UploadResult::RejectedPermissionDenied
            }
            Self::RejectedSymlinkOutput { .. } => bsmr_data::UploadResult::RejectedSymlinkOutput,
            Self::FailedUploadActionBlobs { .. } => {
                bsmr_data::UploadResult::FailedUploadActionBlobs
            }
            Self::FailedUploadOutputs { .. } => bsmr_data::UploadResult::FailedUploadOutputs,
            Self::FailedWriteActionResult { .. } => {
                bsmr_data::UploadResult::FailedWriteActionResult
            }
            Self::FailedOther { .. } => bsmr_data::UploadResult::FailedOther,
            Self::ExecutorUploadDisabled { .. } => bsmr_data::UploadResult::ExecutorUploadDisabled,
            Self::HadDepFileBundle => bsmr_data::UploadResult::HadDepFileBundle,
            Self::NonLocalExecution => bsmr_data::UploadResult::NonLocalExecution,
        }
    }

    pub fn error(&self) -> String {
        match self {
            Self::Success
            | Self::ExecutorUploadDisabled { .. }
            | Self::HadDepFileBundle
            | Self::NonLocalExecution => String::new(),
            Self::RejectedOutputExceedsLimit { max_bytes, .. } => {
                format!("Rejected: OutputExceedsLimit({max_bytes})")
            }
            Self::RejectedPermissionDenied { reason, .. } => {
                format!("Rejected: PermissionDenied (permission check error: {reason})")
            }
            Self::RejectedSymlinkOutput { .. } => "Rejected: SymlinkOutput".to_owned(),
            Self::FailedUploadActionBlobs { error, .. }
            | Self::FailedUploadOutputs { error, .. }
            | Self::FailedWriteActionResult { error, .. }
            | Self::FailedOther { error, .. } => format!("{:#}", error),
        }
    }

    pub fn re_error_code(&self) -> Option<String> {
        match self {
            Self::Success
            | Self::ExecutorUploadDisabled { .. }
            | Self::HadDepFileBundle
            | Self::NonLocalExecution => None,
            Self::RejectedOutputExceedsLimit { .. } | Self::RejectedSymlinkOutput { .. } => None,
            Self::RejectedPermissionDenied { .. } => Some(TCode::PERMISSION_DENIED.to_string()),
            Self::FailedUploadActionBlobs { error, .. }
            | Self::FailedUploadOutputs { error, .. }
            | Self::FailedWriteActionResult { error, .. }
            | Self::FailedOther { error, .. } => {
                match error.find_typed_context::<RemoteExecutionError>() {
                    Some(e) => Some(e.code.to_string()),
                    _ => Some("OTHER_ERRORS".to_owned()),
                }
            }
        }
    }

    pub fn log_and_create_result(
        self,
        digest_str: &str,
        error_on_cache_upload: bool,
    ) -> bsmr_error::Result<Self> {
        match &self {
            Self::Success => {
                info!("Cache upload for `{}` succeeded", digest_str);
            }
            Self::RejectedOutputExceedsLimit { .. }
            | Self::RejectedPermissionDenied { .. }
            | Self::RejectedSymlinkOutput { .. } => {
                info!(
                    "Cache upload for `{}` rejected: {}",
                    digest_str,
                    self.error()
                );
            }
            Self::FailedUploadActionBlobs { .. }
            | Self::FailedUploadOutputs { .. }
            | Self::FailedWriteActionResult { .. }
            | Self::FailedOther { .. } => {
                warn!("Cache upload for `{}` failed: {}", digest_str, self.error());
            }
            Self::ExecutorUploadDisabled { .. }
            | Self::HadDepFileBundle
            | Self::NonLocalExecution => {
                info!("Cache upload for `{}` not attempted", digest_str);
            }
        };
        if !self.uploaded() && error_on_cache_upload {
            Err(bsmr_error::bsmr_error!(
                bsmr_error::ErrorTag::CacheUploadFailed,
                "cache_upload_failed"
            ))
        } else {
            Ok(self)
        }
    }
}

// This is for quick testing of cache upload without configuring executors.
pub fn force_cache_upload() -> bsmr_error::Result<bool> {
    bsmr_env!(
        "BSMR_TEST_FORCE_CACHE_UPLOAD",
        bool,
        applicability = testing
    )
}

/// A single purpose trait to handle cache uploads
#[async_trait]
pub trait UploadCache: Send + Sync {
    /// Whether this uploader is the user-level local action cache.
    fn is_local_action_cache(&self) -> bool {
        false
    }

    /// Given information about the command and its result, upload the result
    /// and related items to the cache.
    /// Return information about why the main action upload did or did not occur.
    async fn upload(
        &self,
        info: &CacheUploadInfo<'_>,
        execution_result: &CommandExecutionResult,
        re_result: Option<TActionResult2>,
        dep_file_bundle: Option<&mut dyn IntoRemoteDepFile>,
        action_digest_and_blobs: &ActionDigestAndBlobs,
    ) -> bsmr_error::Result<CacheUploadResults>;
}

/// A no-op cache uploader for when cache uploading is disabled
pub struct NoOpCacheUploader {}

#[async_trait]
impl UploadCache for NoOpCacheUploader {
    async fn upload(
        &self,
        _info: &CacheUploadInfo<'_>,
        _execution_result: &CommandExecutionResult,
        _re_result: Option<TActionResult2>,
        _dep_file_bundle: Option<&mut dyn IntoRemoteDepFile>,
        _action_digest_and_blobs: &ActionDigestAndBlobs,
    ) -> bsmr_error::Result<CacheUploadResults> {
        Ok(CacheUploadResults {
            cache_upload_outcome: CacheUploadOutcome::ExecutorUploadDisabled,
            dep_file_cache_upload_outcome: DepFileCacheUploadOutcome::NoDepFileBundle,
            dep_file_cache_upload_key: None,
        })
    }
}
