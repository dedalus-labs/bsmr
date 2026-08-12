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

use std::time::Duration;
use std::time::SystemTime;

use bsmr_core::cells::name::CellName;
use bsmr_core::execution_types::executor_config::RemoteExecutorUseCase;
use bsmr_error::BuckErrorContext;
use bsmr_error::internal_error;
use bsmr_fs::paths::forward_rel_path::ForwardRelativePathBuf;
use gazebo::prelude::*;
use host_sharing::HostSharingRequirements;
use host_sharing::WeightClass;
use host_sharing::WeightPercentage;

use super::LocalExecutionCommand;
use super::LocalResourceType;
use super::PrepareForLocalExecutionResult;
use super::RemoteStorageConfig;
use super::RequiredLocalResources;
use super::TtlConfig;
use crate::convert;
use crate::data::ArgHandle;
use crate::data::ArgValue;
use crate::data::ArgValueContent;
use crate::data::ConfiguredTarget;
use crate::data::ConfiguredTargetHandle;
use crate::data::DeclaredOutput;
use crate::data::EnvHandle;
use crate::data::ExecuteRequest2;
use crate::data::ExecutionResult2;
use crate::data::ExecutionStatus;
use crate::data::ExecutionStream;
use crate::data::ExecutorConfigOverride;
use crate::data::ExternalRunnerSpec;
use crate::data::ExternalRunnerSpecValue;
use crate::data::Output;
use crate::data::OutputName;
use crate::data::RemoteDir;
use crate::data::RemoteFile;
use crate::data::RemoteObject;
use crate::data::TestExecutable;
use crate::data::TestResult;
use crate::data::TestStage;
use crate::data::TestStatus;

fn weight_class_from_grpc(
    input: bsmr_host_sharing_proto::WeightClass,
) -> bsmr_error::Result<WeightClass> {
    use bsmr_host_sharing_proto::weight_class::*;

    Ok(
        match input
            .value
            .ok_or_else(|| internal_error!("Missing `value`"))?
        {
            Value::Permits(p) => {
                WeightClass::Permits(p.try_into().buck_error_context("Invalid `permits`")?)
            }
            Value::Percentage(p) => {
                WeightClass::Percentage(WeightPercentage::try_new(p).map_err(|e| {
                    bsmr_error::internal_error!("Invalid `percentage` in grpc: {:#}", e)
                })?)
            }
        },
    )
}

pub fn host_sharing_requirements_from_grpc(
    input: bsmr_host_sharing_proto::HostSharingRequirements,
) -> bsmr_error::Result<HostSharingRequirements> {
    use bsmr_host_sharing_proto::host_sharing_requirements::*;

    let requirements = match input
        .requirements
        .ok_or_else(|| internal_error!("Missing `requirements`"))?
    {
        Requirements::Shared(Shared { weight_class }) => {
            HostSharingRequirements::Shared(weight_class_from_grpc(
                weight_class.ok_or_else(|| internal_error!("Missing `weight_class`"))?,
            )?)
        }
        Requirements::ExclusiveAccess(ExclusiveAccess {}) => {
            HostSharingRequirements::ExclusiveAccess
        }
        Requirements::OnePerToken(OnePerToken {
            identifier,
            weight_class,
        }) => HostSharingRequirements::OnePerToken(
            identifier,
            weight_class_from_grpc(
                weight_class.ok_or_else(|| internal_error!("Missing `weight_class`"))?,
            )?,
        ),
        Requirements::OnePerTokens(OnePerTokens {
            identifiers,
            weight_class,
        }) => HostSharingRequirements::OnePerTokens(
            identifiers.into(),
            weight_class_from_grpc(
                weight_class.ok_or_else(|| internal_error!("Missing `weight_class`"))?,
            )?,
        ),
    };

    Ok(requirements)
}

fn weight_class_to_grpc(
    input: WeightClass,
) -> bsmr_error::Result<bsmr_host_sharing_proto::WeightClass> {
    use bsmr_host_sharing_proto::weight_class::*;

    let value = match input {
        WeightClass::Permits(p) => {
            Value::Permits(p.try_into().buck_error_context("Invalid `permits`")?)
        }
        WeightClass::Percentage(p) => Value::Percentage(p.into_value().into()),
    };

    Ok(bsmr_host_sharing_proto::WeightClass { value: Some(value) })
}

pub fn host_sharing_requirements_to_grpc(
    input: HostSharingRequirements,
) -> bsmr_error::Result<bsmr_host_sharing_proto::HostSharingRequirements> {
    use bsmr_host_sharing_proto::host_sharing_requirements::*;

    let requirements = match input {
        HostSharingRequirements::Shared(weight) => Requirements::Shared(Shared {
            weight_class: Some(weight_class_to_grpc(weight)?),
        }),
        HostSharingRequirements::ExclusiveAccess => {
            Requirements::ExclusiveAccess(ExclusiveAccess {})
        }
        HostSharingRequirements::OnePerToken(identifier, weight) => {
            Requirements::OnePerToken(OnePerToken {
                identifier,
                weight_class: Some(weight_class_to_grpc(weight)?),
            })
        }
        HostSharingRequirements::OnePerTokens(identifiers, weight) => {
            Requirements::OnePerTokens(OnePerTokens {
                identifiers: identifiers.into_iter().collect(),
                weight_class: Some(weight_class_to_grpc(weight)?),
            })
        }
    };

    Ok(bsmr_host_sharing_proto::HostSharingRequirements {
        requirements: Some(requirements),
    })
}

impl TryFrom<bsmr_test_proto::TestStage> for TestStage {
    type Error = bsmr_error::Error;

    fn try_from(s: bsmr_test_proto::TestStage) -> Result<Self, Self::Error> {
        use bsmr_test_proto::Testing;
        use bsmr_test_proto::test_stage::*;

        let res = match s.item.ok_or_else(|| internal_error!("Missing `item`"))? {
            Item::Listing(Listing { suite, cacheable }) => Self::Listing { suite, cacheable },
            Item::Testing(Testing {
                suite,
                testcases,
                variant,
                repeat_count,
            }) => Self::Testing {
                suite,
                testcases,
                variant,
                repeat_count: repeat_count.map(|v| v as usize),
            },
        };

        Ok(res)
    }
}

impl TryInto<bsmr_test_proto::TestStage> for TestStage {
    type Error = bsmr_error::Error;

    fn try_into(self) -> Result<bsmr_test_proto::TestStage, Self::Error> {
        use bsmr_test_proto::Testing;
        use bsmr_test_proto::test_stage::*;

        let item = match self {
            Self::Listing { suite, cacheable } => Item::Listing(Listing { suite, cacheable }),
            Self::Testing {
                suite,
                testcases,
                variant,
                repeat_count,
            } => Item::Testing(Testing {
                suite,
                testcases,
                variant,
                repeat_count: repeat_count.map(|v| v as u64),
            }),
        };

        Ok(bsmr_test_proto::TestStage { item: Some(item) })
    }
}

impl TryFrom<bsmr_test_proto::ExecutionStream> for ExecutionStream {
    type Error = bsmr_error::Error;

    fn try_from(s: bsmr_test_proto::ExecutionStream) -> Result<Self, Self::Error> {
        use bsmr_test_proto::execution_stream::*;

        Ok(
            match s.item.ok_or_else(|| internal_error!("Missing `item`"))? {
                Item::Inline(bytes) => Self::Inline(bytes),
            },
        )
    }
}

impl TryInto<bsmr_test_proto::ExecutionStream> for ExecutionStream {
    type Error = bsmr_error::Error;

    fn try_into(self) -> Result<bsmr_test_proto::ExecutionStream, Self::Error> {
        use bsmr_test_proto::execution_stream::*;

        let item = match self {
            Self::Inline(bytes) => Item::Inline(bytes),
        };

        Ok(bsmr_test_proto::ExecutionStream { item: Some(item) })
    }
}

impl TryFrom<bsmr_test_proto::ExecutionStatus> for ExecutionStatus {
    type Error = bsmr_error::Error;

    fn try_from(s: bsmr_test_proto::ExecutionStatus) -> Result<Self, Self::Error> {
        use bsmr_test_proto::execution_status::*;

        Ok(
            match s
                .status
                .ok_or_else(|| internal_error!("Missing `status`"))?
            {
                Status::Finished(exitcode) => Self::Finished { exitcode },
                Status::TimedOut(duration) => Self::TimedOut {
                    duration: convert::to_std_duration(duration)?,
                },
            },
        )
    }
}

impl TryInto<bsmr_test_proto::ExecutionStatus> for ExecutionStatus {
    type Error = bsmr_error::Error;

    fn try_into(self) -> Result<bsmr_test_proto::ExecutionStatus, Self::Error> {
        use bsmr_test_proto::execution_status::*;

        let status = match self {
            Self::Finished { exitcode } => Status::Finished(exitcode),
            Self::TimedOut { duration } => Status::TimedOut(duration.try_into()?),
        };

        Ok(bsmr_test_proto::ExecutionStatus {
            status: Some(status),
        })
    }
}

impl TryFrom<bsmr_test_proto::ConfiguredTargetHandle> for ConfiguredTargetHandle {
    type Error = bsmr_error::Error;

    fn try_from(s: bsmr_test_proto::ConfiguredTargetHandle) -> Result<Self, Self::Error> {
        let handle = s.id.try_into().buck_error_context("Invalid `id`")?;
        Ok(Self(handle))
    }
}

impl TryInto<bsmr_test_proto::ConfiguredTargetHandle> for ConfiguredTargetHandle {
    type Error = bsmr_error::Error;

    fn try_into(self) -> Result<bsmr_test_proto::ConfiguredTargetHandle, Self::Error> {
        Ok(bsmr_test_proto::ConfiguredTargetHandle {
            id: self.0.try_into().buck_error_context("Invalid `handle`")?,
        })
    }
}

impl TryFrom<bsmr_test_proto::ConfiguredTarget> for ConfiguredTarget {
    type Error = bsmr_error::Error;

    fn try_from(s: bsmr_test_proto::ConfiguredTarget) -> Result<Self, Self::Error> {
        let bsmr_test_proto::ConfiguredTarget {
            handle,
            cell,
            package,
            target,
            configuration,
            package_project_relative_path,
            test_config_unification_rollout,
            package_oncall,
        } = s;

        Ok(Self {
            handle: handle
                .ok_or_else(|| internal_error!("Missing `handle`"))?
                .try_into()
                .buck_error_context("Invalid `handle`")?,
            cell,
            package,
            target,
            configuration,
            package_project_relative_path: ForwardRelativePathBuf::try_from(
                package_project_relative_path,
            )?,
            test_config_unification_rollout,
            package_oncall,
        })
    }
}

impl TryInto<bsmr_test_proto::ConfiguredTarget> for ConfiguredTarget {
    type Error = bsmr_error::Error;

    fn try_into(self) -> Result<bsmr_test_proto::ConfiguredTarget, Self::Error> {
        Ok(bsmr_test_proto::ConfiguredTarget {
            handle: Some(
                self.handle
                    .try_into()
                    .buck_error_context("Invalid `handle`")?,
            ),
            cell: self.cell,
            package: self.package,
            target: self.target,
            configuration: self.configuration,
            package_project_relative_path: self.package_project_relative_path.as_str().to_owned(),
            test_config_unification_rollout: self.test_config_unification_rollout,
            package_oncall: self.package_oncall,
        })
    }
}

impl TryFrom<i32> for TestStatus {
    type Error = bsmr_error::Error;

    fn try_from(s: i32) -> Result<Self, Self::Error> {
        let s = bsmr_test_proto::TestStatus::try_from(s).buck_error_context("Invalid `status`")?;

        Ok(match s {
            bsmr_test_proto::TestStatus::NotSet => {
                return Err(bsmr_error::internal_error!("Missing `status`"));
            }
            bsmr_test_proto::TestStatus::Pass => TestStatus::PASS,
            bsmr_test_proto::TestStatus::Fail => TestStatus::FAIL,
            bsmr_test_proto::TestStatus::Skip => TestStatus::SKIP,
            bsmr_test_proto::TestStatus::Omitted => TestStatus::OMITTED,
            bsmr_test_proto::TestStatus::Fatal => TestStatus::FATAL,
            bsmr_test_proto::TestStatus::Timeout => TestStatus::TIMEOUT,
            bsmr_test_proto::TestStatus::InfraFailure => TestStatus::INFRA_FAILURE,
            bsmr_test_proto::TestStatus::Unknown => TestStatus::UNKNOWN,
            bsmr_test_proto::TestStatus::Rerun => TestStatus::RERUN,
            bsmr_test_proto::TestStatus::ListingSuccess => TestStatus::LISTING_SUCCESS,
            bsmr_test_proto::TestStatus::ListingFailed => TestStatus::LISTING_FAILED,
        })
    }
}

impl TryInto<i32> for TestStatus {
    type Error = bsmr_error::Error;

    fn try_into(self) -> Result<i32, Self::Error> {
        Ok(match self {
            TestStatus::PASS => bsmr_test_proto::TestStatus::Pass,
            TestStatus::FAIL => bsmr_test_proto::TestStatus::Fail,
            TestStatus::SKIP => bsmr_test_proto::TestStatus::Skip,
            TestStatus::OMITTED => bsmr_test_proto::TestStatus::Omitted,
            TestStatus::FATAL => bsmr_test_proto::TestStatus::Fatal,
            TestStatus::TIMEOUT => bsmr_test_proto::TestStatus::Timeout,
            TestStatus::INFRA_FAILURE => bsmr_test_proto::TestStatus::InfraFailure,
            TestStatus::UNKNOWN => bsmr_test_proto::TestStatus::Unknown,
            TestStatus::RERUN => bsmr_test_proto::TestStatus::Rerun,
            TestStatus::LISTING_SUCCESS => bsmr_test_proto::TestStatus::ListingSuccess,
            TestStatus::LISTING_FAILED => bsmr_test_proto::TestStatus::ListingFailed,
        } as i32)
    }
}

impl TryFrom<bsmr_test_proto::TestResult> for TestResult {
    type Error = bsmr_error::Error;

    fn try_from(s: bsmr_test_proto::TestResult) -> Result<Self, Self::Error> {
        let bsmr_test_proto::TestResult {
            target,
            name,
            status,
            msg,
            duration,
            details,
            max_memory_used_bytes,
            attempt,
        } = s;

        let duration = duration
            .map(convert::to_std_duration)
            .transpose()
            .buck_error_context("For `duration`")?;

        Ok(Self {
            target: target
                .ok_or_else(|| internal_error!("Missing `target`"))?
                .try_into()
                .buck_error_context("Invalid `target`")?,
            name,
            status: status.try_into().buck_error_context("Invalid `status`")?,
            msg: msg.map(|m| m.msg),
            duration,
            max_memory_used_bytes,
            details,
            attempt,
        })
    }
}

impl TryInto<bsmr_test_proto::TestResult> for TestResult {
    type Error = bsmr_error::Error;

    fn try_into(self) -> Result<bsmr_test_proto::TestResult, Self::Error> {
        use bsmr_test_proto::test_result::*;

        Ok(bsmr_test_proto::TestResult {
            target: Some(
                self.target
                    .try_into()
                    .buck_error_context("Invalid `target`")?,
            ),
            name: self.name,
            status: self
                .status
                .try_into()
                .buck_error_context("Invalid `status`")?,
            details: self.details,
            msg: self.msg.map(|msg| OptionalMsg { msg }),
            duration: self.duration.try_map(|d| d.try_into())?,
            max_memory_used_bytes: self.max_memory_used_bytes,
            attempt: self.attempt,
        })
    }
}

impl TryFrom<bsmr_test_proto::ExternalRunnerSpec> for ExternalRunnerSpec {
    type Error = bsmr_error::Error;

    fn try_from(s: bsmr_test_proto::ExternalRunnerSpec) -> Result<Self, Self::Error> {
        let bsmr_test_proto::ExternalRunnerSpec {
            target,
            test_type,
            command,
            env,
            labels,
            contacts,
            oncall,
            working_dir_cell,
        } = s;

        Ok(Self {
            target: target
                .ok_or_else(|| internal_error!("Missing `target`"))?
                .try_into()
                .buck_error_context("Invalid `target`")?,
            test_type,
            command: command
                .into_try_map(|x| x.try_into())
                .buck_error_context("Invalid `command`")?,
            env: env
                .into_iter()
                .map(|(k, v)| Ok((k, v.try_into().unwrap())))
                .collect::<Result<_, Self::Error>>()?,
            labels,
            contacts,
            oncall,
            working_dir_cell: CellName::unchecked_new(&working_dir_cell)?,
        })
    }
}

impl TryInto<bsmr_test_proto::ExternalRunnerSpec> for ExternalRunnerSpec {
    type Error = bsmr_error::Error;

    fn try_into(self) -> Result<bsmr_test_proto::ExternalRunnerSpec, Self::Error> {
        let ExternalRunnerSpec {
            target,
            test_type,
            command,
            env,
            labels,
            contacts,
            oncall,
            working_dir_cell,
        } = self;
        Ok(bsmr_test_proto::ExternalRunnerSpec {
            target: Some(target.try_into().buck_error_context("Invalid `target`")?),
            test_type,
            command: command
                .into_try_map(|x| x.try_into())
                .buck_error_context("Invalid `command`")?,
            env: env
                .into_iter()
                .map(|(k, v)| Ok((k, v.try_into().unwrap())))
                .collect::<Result<_, Self::Error>>()?,
            labels,
            contacts,
            oncall,
            working_dir_cell: working_dir_cell.as_str().to_owned(),
        })
    }
}

impl TryFrom<bsmr_test_proto::ExternalRunnerSpecValue> for ExternalRunnerSpecValue {
    type Error = bsmr_error::Error;

    fn try_from(s: bsmr_test_proto::ExternalRunnerSpecValue) -> Result<Self, Self::Error> {
        use bsmr_test_proto::external_runner_spec_value::*;
        Ok(
            match s.value.ok_or_else(|| internal_error!("Missing `value`"))? {
                Value::Verbatim(val) => ExternalRunnerSpecValue::Verbatim(val),
                Value::ArgHandle(val) => ExternalRunnerSpecValue::ArgHandle(
                    val.try_into().buck_error_context("Invalid `arg_handle`")?,
                ),
                Value::EnvHandle(val) => ExternalRunnerSpecValue::EnvHandle(val.into()),
            },
        )
    }
}

impl TryInto<bsmr_test_proto::ExternalRunnerSpecValue> for ExternalRunnerSpecValue {
    type Error = bsmr_error::Error;

    fn try_into(self) -> Result<bsmr_test_proto::ExternalRunnerSpecValue, Self::Error> {
        use bsmr_test_proto::external_runner_spec_value::*;

        let value = match self {
            Self::Verbatim(val) => Value::Verbatim(val),
            Self::ArgHandle(ArgHandle(val)) => {
                Value::ArgHandle(val.try_into().buck_error_context("Invalid `arg_handle`")?)
            }
            Self::EnvHandle(EnvHandle(val)) => Value::EnvHandle(val),
        };

        Ok(bsmr_test_proto::ExternalRunnerSpecValue { value: Some(value) })
    }
}

impl From<OutputName> for bsmr_test_proto::OutputName {
    fn from(o: OutputName) -> Self {
        Self {
            name: o.name.as_str().to_owned(),
        }
    }
}

impl TryFrom<bsmr_test_proto::OutputName> for OutputName {
    type Error = bsmr_error::Error;

    fn try_from(o: bsmr_test_proto::OutputName) -> Result<Self, Self::Error> {
        let name = ForwardRelativePathBuf::try_from(o.name)?;
        Ok(Self { name })
    }
}

impl From<TtlConfig> for bsmr_test_proto::TtlConfig {
    fn from(o: TtlConfig) -> Self {
        Self {
            ttl_seconds: o.ttl.as_secs() as i64,
            use_case: o.use_case.to_string(),
        }
    }
}

impl From<bsmr_test_proto::TtlConfig> for TtlConfig {
    fn from(o: bsmr_test_proto::TtlConfig) -> Self {
        let ttl = Duration::from_secs(o.ttl_seconds as u64);
        let use_case = RemoteExecutorUseCase::new(o.use_case);
        Self { ttl, use_case }
    }
}

impl From<DeclaredOutput> for bsmr_test_proto::DeclaredOutput {
    fn from(o: DeclaredOutput) -> Self {
        Self {
            name: o.name.as_str().to_owned(),
            supports_remote: o.remote_storage_config.supports_remote,
            ttl_config: o.remote_storage_config.ttl_config.map(Into::into),
        }
    }
}

impl TryFrom<bsmr_test_proto::DeclaredOutput> for DeclaredOutput {
    type Error = bsmr_error::Error;

    fn try_from(o: bsmr_test_proto::DeclaredOutput) -> Result<Self, Self::Error> {
        let name = ForwardRelativePathBuf::try_from(o.name)?.into();
        let remote_storage_config = RemoteStorageConfig {
            supports_remote: o.supports_remote,
            ttl_config: o.ttl_config.map(Into::into),
        };
        Ok(Self {
            name,
            remote_storage_config,
        })
    }
}

impl From<ExecutorConfigOverride> for bsmr_test_proto::ExecutorConfigOverride {
    fn from(o: ExecutorConfigOverride) -> Self {
        Self {
            name: o.name.as_str().to_owned(),
        }
    }
}

impl From<bsmr_test_proto::ExecutorConfigOverride> for ExecutorConfigOverride {
    fn from(o: bsmr_test_proto::ExecutorConfigOverride) -> Self {
        Self { name: o.name }
    }
}

impl From<LocalResourceType> for bsmr_test_proto::LocalResourceType {
    fn from(r: LocalResourceType) -> Self {
        Self {
            name: r.name.as_str().to_owned(),
        }
    }
}

impl From<bsmr_test_proto::LocalResourceType> for LocalResourceType {
    fn from(o: bsmr_test_proto::LocalResourceType) -> Self {
        Self { name: o.name }
    }
}

impl TryInto<bsmr_test_proto::ArgValue> for ArgValue {
    type Error = bsmr_error::Error;

    fn try_into(self) -> Result<bsmr_test_proto::ArgValue, Self::Error> {
        Ok(bsmr_test_proto::ArgValue {
            content: Some(
                self.content
                    .try_into()
                    .buck_error_context("Invalid `content`")?,
            ),
            format: self
                .format
                .map(|f| bsmr_test_proto::ArgFormat { format: f }),
        })
    }
}

impl TryFrom<bsmr_test_proto::ArgValue> for ArgValue {
    type Error = bsmr_error::Error;

    fn try_from(s: bsmr_test_proto::ArgValue) -> Result<Self, Self::Error> {
        let content = s
            .content
            .ok_or_else(|| internal_error!("Missing `content`"))?
            .try_into()
            .buck_error_context("Invalid `content`")?;
        let format = s.format.map(|f| f.format);

        Ok(Self { content, format })
    }
}

impl TryInto<bsmr_test_proto::ArgValueContent> for ArgValueContent {
    type Error = bsmr_error::Error;

    fn try_into(self) -> Result<bsmr_test_proto::ArgValueContent, Self::Error> {
        use bsmr_test_proto::arg_value_content::*;

        let value = match self {
            Self::ExternalRunnerSpecValue(value) => Value::SpecValue(
                value
                    .try_into()
                    .buck_error_context("Invalid external runner spec value")?,
            ),
            Self::DeclaredOutput(value) => Value::DeclaredOutput(value.into()),
        };

        Ok(bsmr_test_proto::ArgValueContent { value: Some(value) })
    }
}

impl TryFrom<bsmr_test_proto::ArgValueContent> for ArgValueContent {
    type Error = bsmr_error::Error;

    fn try_from(s: bsmr_test_proto::ArgValueContent) -> Result<Self, Self::Error> {
        use bsmr_test_proto::arg_value_content::*;

        Ok(
            match s.value.ok_or_else(|| internal_error!("Missing `value`"))? {
                Value::SpecValue(value) => Self::ExternalRunnerSpecValue(
                    value
                        .try_into()
                        .buck_error_context("Invalid external runner spec value")?,
                ),
                Value::DeclaredOutput(value) => {
                    Self::DeclaredOutput(value.try_into().buck_error_context("Invalid `value`")?)
                }
            },
        )
    }
}

impl TryFrom<bsmr_test_proto::ExecuteRequest2> for ExecuteRequest2 {
    type Error = bsmr_error::Error;

    fn try_from(s: bsmr_test_proto::ExecuteRequest2) -> Result<Self, Self::Error> {
        let bsmr_test_proto::ExecuteRequest2 {
            test_executable,
            timeout,
            host_sharing_requirements,
            executor_override,
            required_local_resources,
            disable_test_execution_caching,
        } = s;

        let test_executable = test_executable
            .ok_or_else(|| internal_error!("Missing `test_executable`"))?
            .try_into()
            .buck_error_context("Invalid `test_executable`")?;

        let timeout =
            convert::to_std_duration(timeout.ok_or_else(|| internal_error!("Missing `timeout`"))?)
                .buck_error_context("Invalid `timeout`")?;

        let host_sharing_requirements = host_sharing_requirements
            .ok_or_else(|| internal_error!("Missing `host_sharing_requirements`"))?;
        let host_sharing_requirements =
            host_sharing_requirements_from_grpc(host_sharing_requirements)
                .buck_error_context("Invalid `host_sharing_requirements`")?;

        let executor_override = executor_override.map(|o| o.into());

        let required_local_resources = RequiredLocalResources {
            resources: required_local_resources.into_map(|r| r.into()),
        };

        Ok(ExecuteRequest2 {
            test_executable,
            timeout,
            host_sharing_requirements,
            executor_override,
            required_local_resources,
            disable_test_execution_caching,
        })
    }
}

impl TryInto<bsmr_test_proto::ExecuteRequest2> for ExecuteRequest2 {
    type Error = bsmr_error::Error;

    fn try_into(self) -> Result<bsmr_test_proto::ExecuteRequest2, Self::Error> {
        let test_executable = Some(
            self.test_executable
                .try_into()
                .buck_error_context("Invalid `test_executable`")?,
        );

        Ok(bsmr_test_proto::ExecuteRequest2 {
            test_executable,
            timeout: Some(self.timeout.try_into()?),
            host_sharing_requirements: Some(
                host_sharing_requirements_to_grpc(self.host_sharing_requirements)
                    .buck_error_context("Invalid `host_sharing_requirements`")?,
            ),
            executor_override: self.executor_override.map(|o| o.into()),
            required_local_resources: self
                .required_local_resources
                .resources
                .into_map(|r| r.into()),
            disable_test_execution_caching: self.disable_test_execution_caching,
        })
    }
}

impl TryInto<bsmr_test_proto::RemoteObject> for RemoteObject {
    type Error = bsmr_error::Error;

    fn try_into(self) -> Result<bsmr_test_proto::RemoteObject, Self::Error> {
        match self {
            RemoteObject::File(RemoteFile { name, digest }) => {
                let node = bsmr_test_proto::RemoteFileNode { name };
                Ok(bsmr_test_proto::RemoteObject {
                    digest: Some(digest),
                    node: Some(bsmr_test_proto::remote_object::Node::File(node)),
                })
            }
            RemoteObject::Dir(RemoteDir {
                name,
                digest,
                children,
            }) => {
                let children = children
                    .into_iter()
                    .map(|child| child.try_into())
                    .collect::<Result<Vec<_>, _>>()?;
                let node = bsmr_test_proto::RemoteDirNode { name, children };
                Ok(bsmr_test_proto::RemoteObject {
                    digest: Some(digest),
                    node: Some(bsmr_test_proto::remote_object::Node::Dir(node)),
                })
            }
        }
    }
}

impl TryFrom<bsmr_test_proto::RemoteObject> for RemoteObject {
    type Error = bsmr_error::Error;

    fn try_from(value: bsmr_test_proto::RemoteObject) -> Result<Self, Self::Error> {
        let digest = value
            .digest
            .ok_or_else(|| internal_error!("missing digest"))?;
        match value.node.ok_or_else(|| internal_error!("missing node"))? {
            bsmr_test_proto::remote_object::Node::File(file) => {
                Ok(RemoteObject::file(file.name, digest))
            }
            bsmr_test_proto::remote_object::Node::Dir(dir) => {
                let children = dir
                    .children
                    .into_iter()
                    .map(|child| child.try_into())
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(RemoteObject::dir(dir.name, digest, children))
            }
        }
    }
}

impl TryInto<bsmr_test_proto::Output> for Output {
    type Error = bsmr_error::Error;

    fn try_into(self) -> Result<bsmr_test_proto::Output, Self::Error> {
        use bsmr_test_proto::output::*;

        let value = match self {
            Self::LocalPath(value) => Value::LocalPath(
                value
                    .to_str()
                    .buck_error_context("Invalid local path")?
                    .to_owned(),
            ),
            Self::RemoteObject(value) => Value::RemoteObject(value.try_into()?),
        };

        Ok(bsmr_test_proto::Output { value: Some(value) })
    }
}

impl TryFrom<bsmr_test_proto::Output> for Output {
    type Error = bsmr_error::Error;

    fn try_from(s: bsmr_test_proto::Output) -> Result<Self, Self::Error> {
        use bsmr_test_proto::output::*;

        Ok(
            match s.value.ok_or_else(|| internal_error!("Missing `value`"))? {
                Value::LocalPath(value) => Self::LocalPath(
                    value
                        .try_into()
                        .buck_error_context("Invalid local path value.")?,
                ),
                Value::RemoteObject(value) => Self::RemoteObject(value.try_into()?),
            },
        )
    }
}

impl TryInto<bsmr_test_proto::ExecutionResult2> for ExecutionResult2 {
    type Error = bsmr_error::Error;

    fn try_into(self) -> Result<bsmr_test_proto::ExecutionResult2, Self::Error> {
        Ok(bsmr_test_proto::ExecutionResult2 {
            status: Some(
                self.status
                    .try_into()
                    .buck_error_context("Invalid `status`")?,
            ),
            stdout: Some(
                self.stdout
                    .try_into()
                    .buck_error_context("Invalid `stdout`")?,
            ),
            stderr: Some(
                self.stderr
                    .try_into()
                    .buck_error_context("Invalid `stderr`")?,
            ),
            outputs: self
                .outputs
                .into_iter()
                .map(|(k, v)| {
                    Ok(bsmr_test_proto::OutputEntry {
                        declared_output: Some(k.into()),
                        output: Some(v.try_into().buck_error_context("Invalid `output`")?),
                    })
                })
                .collect::<Result<_, Self::Error>>()?,
            start_time: Some(
                self.start_time
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap_or_default()
                    .try_into()?,
            ),
            execution_time: Some(self.execution_time.try_into()?),
            execution_details: Some(self.execution_details),
            max_memory_used_bytes: self.max_memory_used_bytes,
            command_execution: self.command_execution,
        })
    }
}

impl TryFrom<bsmr_test_proto::ExecutionResult2> for ExecutionResult2 {
    type Error = bsmr_error::Error;

    fn try_from(s: bsmr_test_proto::ExecutionResult2) -> Result<Self, Self::Error> {
        let bsmr_test_proto::ExecutionResult2 {
            status,
            stdout,
            stderr,
            outputs,
            start_time,
            execution_time,
            execution_details,
            max_memory_used_bytes,
            command_execution,
        } = s;
        let status = status
            .ok_or_else(|| internal_error!("Missing `status`"))?
            .try_into()
            .buck_error_context("Invalid `status`")?;
        let stdout = stdout
            .ok_or_else(|| internal_error!("Missing `stdout`"))?
            .try_into()
            .buck_error_context("Invalid `stdout`")?;
        let stderr = stderr
            .ok_or_else(|| internal_error!("Missing `stderr`"))?
            .try_into()
            .buck_error_context("Invalid `stderr`")?;

        let outputs = outputs
            .into_iter()
            .map(|entry| {
                let bsmr_test_proto::OutputEntry {
                    declared_output,
                    output,
                } = entry;
                let declared_output = declared_output
                    .ok_or_else(|| internal_error!("Missing `declared_output`"))?
                    .try_into()
                    .buck_error_context("Invalid `declared_output`")?;
                let output = output
                    .ok_or_else(|| internal_error!("Missing `output`"))?
                    .try_into()
                    .buck_error_context("Invalid `output`")?;
                Ok((declared_output, output))
            })
            .collect::<Result<_, Self::Error>>()?;

        let start_time = SystemTime::UNIX_EPOCH
            + convert::to_std_duration(
                start_time.ok_or_else(|| internal_error!("Missing `start_time`"))?,
            )
            .buck_error_context("Invalid `start_time`")?;

        let execution_time = convert::to_std_duration(
            execution_time.ok_or_else(|| internal_error!("Missing `execution_time`"))?,
        )
        .buck_error_context("Invalid `execution_time`")?;

        let execution_details =
            execution_details.ok_or_else(|| internal_error!("Missing `execution_details`"))?;

        Ok(ExecutionResult2 {
            status,
            stdout,
            stderr,
            outputs,
            start_time,
            execution_time,
            max_memory_used_bytes,
            execution_details,
            command_execution,
        })
    }
}

impl TryFrom<bsmr_test_proto::TestExecutable> for TestExecutable {
    type Error = bsmr_error::Error;

    fn try_from(s: bsmr_test_proto::TestExecutable) -> Result<Self, Self::Error> {
        let bsmr_test_proto::TestExecutable {
            stage,
            target,
            cmd,
            pre_create_dirs,
            env,
        } = s;
        let ui_prints = stage
            .ok_or_else(|| internal_error!("Missing `ui_prints`"))?
            .try_into()
            .buck_error_context("Invalid `ui_prints`")?;

        let target = target
            .ok_or_else(|| internal_error!("Missing `target`"))?
            .try_into()
            .buck_error_context("Invalid `target`")?;

        let cmd = cmd
            .into_try_map(|c| c.try_into())
            .buck_error_context("Invalid `cmd`")?;

        let env = env
            .into_iter()
            .map(|env_var| {
                let bsmr_test_proto::EnvironmentVariable { key, value } = env_var;
                value
                    .ok_or_else(|| internal_error!("Missing `value`"))?
                    .try_into()
                    .buck_error_context("Invalid `env`")
                    .map(|v: ArgValue| (key, v))
            })
            .collect::<bsmr_error::Result<_>>()?;

        let pre_create_dirs = pre_create_dirs
            .into_try_map(|c| c.try_into())
            .buck_error_context("Invalid `pre_create_dirs`")?;

        Ok(TestExecutable {
            stage: ui_prints,
            target,
            cmd,
            env,
            pre_create_dirs,
        })
    }
}

impl TryInto<bsmr_test_proto::TestExecutable> for TestExecutable {
    type Error = bsmr_error::Error;

    fn try_into(self) -> Result<bsmr_test_proto::TestExecutable, Self::Error> {
        let stage = Some(
            self.stage
                .try_into()
                .buck_error_context("Invalid `ui_prints`")?,
        );
        let target = Some(
            self.target
                .try_into()
                .buck_error_context("Invalid `target`")?,
        );
        let cmd = self
            .cmd
            .into_try_map(|i| i.try_into())
            .buck_error_context("Invalid `cmd`")?;

        let env = self
            .env
            .into_iter()
            .map(|(k, v)| {
                v.try_into().buck_error_context("Invalid `env`").map(
                    |v: bsmr_test_proto::ArgValue| bsmr_test_proto::EnvironmentVariable {
                        key: k,
                        value: Some(v),
                    },
                )
            })
            .collect::<bsmr_error::Result<_>>()?;

        let pre_create_dirs = self.pre_create_dirs.into_map(|i| i.into());

        Ok(bsmr_test_proto::TestExecutable {
            stage,
            target,
            cmd,
            pre_create_dirs,
            env,
        })
    }
}

impl TryInto<bsmr_test_proto::PrepareForLocalExecutionResponse> for PrepareForLocalExecutionResult {
    type Error = bsmr_error::Error;

    fn try_into(self) -> Result<bsmr_test_proto::PrepareForLocalExecutionResponse, Self::Error> {
        let cwd = self
            .command
            .cwd
            .to_str()
            .buck_error_context("Invalid cwd path")?
            .to_owned();

        Ok(bsmr_test_proto::PrepareForLocalExecutionResponse {
            result: Some(bsmr_test_proto::PrepareForLocalExecutionResult {
                cmd: self.command.cmd,
                cwd,
                env: self
                    .command
                    .env
                    .into_iter()
                    .map(|(key, value)| bsmr_test_proto::VerbatimEnvironmentVariable { key, value })
                    .collect(),
            }),
            setup_local_resource_commands: self
                .local_resource_setup_commands
                .into_iter()
                .map(|c| {
                    <LocalExecutionCommand as TryInto<
                        bsmr_test_proto::SetupLocalResourceLocalExecutionCommand,
                    >>::try_into(c)
                })
                .collect::<Result<Vec<_>, bsmr_error::Error>>()?,
        })
    }
}

impl TryInto<bsmr_test_proto::SetupLocalResourceLocalExecutionCommand> for LocalExecutionCommand {
    type Error = bsmr_error::Error;

    fn try_into(
        self,
    ) -> Result<bsmr_test_proto::SetupLocalResourceLocalExecutionCommand, Self::Error> {
        Ok(bsmr_test_proto::SetupLocalResourceLocalExecutionCommand {
            cmd: self.cmd,
            cwd: self
                .cwd
                .to_str()
                .buck_error_context("Invalid cwd path for local resource")?
                .to_owned(),
            env: self
                .env
                .into_iter()
                .map(|(k, v)| bsmr_test_proto::VerbatimEnvironmentVariable { key: k, value: v })
                .collect(),
        })
    }
}

impl TryFrom<bsmr_test_proto::PrepareForLocalExecutionResult> for LocalExecutionCommand {
    type Error = bsmr_error::Error;

    fn try_from(s: bsmr_test_proto::PrepareForLocalExecutionResult) -> Result<Self, Self::Error> {
        Ok(Self {
            cmd: s.cmd,
            cwd: s.cwd.try_into().buck_error_context("Invalid cwd value.")?,
            env: s
                .env
                .into_iter()
                .map(|env_var| (env_var.key, env_var.value))
                .collect(),
        })
    }
}

impl TryFrom<bsmr_test_proto::SetupLocalResourceLocalExecutionCommand> for LocalExecutionCommand {
    type Error = bsmr_error::Error;

    fn try_from(
        s: bsmr_test_proto::SetupLocalResourceLocalExecutionCommand,
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            cmd: s.cmd,
            cwd: s.cwd.try_into().buck_error_context("Invalid cwd value.")?,
            env: s
                .env
                .into_iter()
                .map(|env_var| (env_var.key, env_var.value))
                .collect(),
        })
    }
}

impl TryFrom<bsmr_test_proto::PrepareForLocalExecutionResponse> for PrepareForLocalExecutionResult {
    type Error = bsmr_error::Error;

    fn try_from(s: bsmr_test_proto::PrepareForLocalExecutionResponse) -> Result<Self, Self::Error> {
        let result = s
            .result
            .ok_or_else(|| internal_error!("Missing `result`"))?;
        Ok(Self {
            command: LocalExecutionCommand::try_from(result)?,
            local_resource_setup_commands: s
                .setup_local_resource_commands
                .into_iter()
                .map(LocalExecutionCommand::try_from)
                .collect::<Result<Vec<_>, bsmr_error::Error>>()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::fmt::Debug;

    use host_sharing::HostSharingRequirements;
    use sorted_vector_map::sorted_vector_map;

    use super::*;

    fn assert_roundtrips<P, S>(s: &S)
    where
        S: Clone + PartialEq + Debug,
        P: TryInto<S, Error = bsmr_error::Error>,
        S: TryInto<P, Error = bsmr_error::Error>,
    {
        let proto: P = s.clone().try_into().unwrap();
        let roundtrip: S = proto.try_into().unwrap();
        assert_eq!(*s, roundtrip);
    }

    #[test]
    fn external_runner_spec_roundtrip() {
        let test_spec = ExternalRunnerSpec {
            target: ConfiguredTarget {
                handle: ConfiguredTargetHandle(1),
                cell: "qux".into(),
                package: "foo".into(),
                target: "bar".into(),
                configuration: "xxx".into(),
                package_project_relative_path: ForwardRelativePathBuf::unchecked_new(
                    "qux/foo".to_owned(),
                ),
                test_config_unification_rollout: false,
                package_oncall: None,
            },
            test_type: "some_type".to_owned(),
            command: vec![
                ExternalRunnerSpecValue::Verbatim("arg".to_owned()),
                ExternalRunnerSpecValue::ArgHandle(ArgHandle(42)),
            ],
            env: [
                (
                    "FOO".to_owned(),
                    ExternalRunnerSpecValue::EnvHandle(EnvHandle("FOO".to_owned())),
                ),
                (
                    "BAR".to_owned(),
                    ExternalRunnerSpecValue::Verbatim("BAR".to_owned()),
                ),
            ]
            .into_iter()
            .collect(),
            labels: vec!["label1".to_owned(), "label2".to_owned()],
            contacts: vec!["contact1".to_owned(), "contact2".to_owned()],
            oncall: Some("contact1".to_owned()),
            working_dir_cell: CellName::testing_new("qux"),
        };
        assert_roundtrips::<bsmr_test_proto::ExternalRunnerSpec, ExternalRunnerSpec>(&test_spec);
    }

    #[test]
    fn execute_request2_roundtrip() {
        let declared_output = DeclaredOutput {
            name: OutputName::unchecked_new("name".to_owned()),
            remote_storage_config: RemoteStorageConfig::new(true),
        };

        let test_executable = TestExecutable {
            stage: TestStage::Listing {
                suite: "name".to_owned(),
                cacheable: true,
            },
            target: ConfiguredTargetHandle(42),
            cmd: vec![
                ArgValue {
                    content: ArgValueContent::ExternalRunnerSpecValue(
                        ExternalRunnerSpecValue::Verbatim("arg".to_owned()),
                    ),
                    format: None,
                },
                ArgValue {
                    content: ArgValueContent::DeclaredOutput(declared_output.name.clone()),
                    format: Some("--output={}".to_owned()),
                },
            ],
            env: [(
                "FOO".to_owned(),
                ArgValue {
                    content: ArgValueContent::ExternalRunnerSpecValue(
                        ExternalRunnerSpecValue::EnvHandle(EnvHandle("FOO".to_owned())),
                    ),
                    format: None,
                },
            )]
            .into_iter()
            .collect(),
            pre_create_dirs: vec![declared_output],
        };
        let request = ExecuteRequest2 {
            test_executable,
            timeout: Duration::from_millis(42),
            host_sharing_requirements: HostSharingRequirements::ExclusiveAccess,
            executor_override: Some(ExecutorConfigOverride {
                name: "foo".to_owned(),
            }),
            required_local_resources: RequiredLocalResources { resources: vec![] },
            disable_test_execution_caching: true,
        };
        assert_roundtrips::<bsmr_test_proto::ExecuteRequest2, ExecuteRequest2>(&request);
    }

    #[test]
    fn execution_result2_roundtrips() {
        let local_path = if cfg!(not(windows)) {
            "/some/path"
        } else {
            "c:/some/path"
        };

        let result = ExecutionResult2 {
            status: ExecutionStatus::Finished { exitcode: 42 },
            stdout: ExecutionStream::Inline(vec![97, 115, 109]),
            stderr: ExecutionStream::Inline(vec![118, 105, 109]),
            outputs: [(
                OutputName::unchecked_new("name".to_owned()),
                Output::LocalPath(String::from(local_path).try_into().expect("valid abs path")),
            )]
            .into_iter()
            .collect(),
            start_time: SystemTime::UNIX_EPOCH + Duration::from_secs(123),
            execution_time: Duration::from_secs(456),
            execution_details: Default::default(),
            max_memory_used_bytes: None,
            command_execution: None,
        };
        assert_roundtrips::<bsmr_test_proto::ExecutionResult2, ExecutionResult2>(&result);
    }

    /// Verifies that action identity survives the test-result RPC boundary.
    #[test]
    fn test_result_attempt_roundtrips() {
        let result = TestResult {
            target: ConfiguredTargetHandle(42),
            name: "case".to_owned(),
            status: TestStatus::PASS,
            msg: None,
            duration: Some(Duration::from_millis(7)),
            max_memory_used_bytes: None,
            details: String::new(),
            attempt: Some(bsmr_data::TestAttempt {
                action_digest: "action:42".to_owned(),
                suite: "unit".to_owned(),
                variant: None,
                attempt: 1,
                execution_kind: bsmr_data::ActionExecutionKind::Local.into(),
            }),
        };

        assert_roundtrips::<bsmr_test_proto::TestResult, TestResult>(&result);
    }

    fn dummy_local_execution_command() -> LocalExecutionCommand {
        let cmd = vec![
            "my_cmd".to_owned(),
            "--some-arg".to_owned(),
            "some_value".to_owned(),
        ];
        let local_path = if cfg!(not(windows)) {
            "/some/path"
        } else {
            "c:/some/path"
        };
        let cwd = String::from(local_path).try_into().expect("valid abs path");
        let env = sorted_vector_map! { "some_env".to_owned() => "some_env_val".to_owned() };

        LocalExecutionCommand { cmd, env, cwd }
    }

    #[test]
    fn prepare_for_local_execution_result_roundtrip() {
        let result = PrepareForLocalExecutionResult {
            command: dummy_local_execution_command(),
            local_resource_setup_commands: vec![dummy_local_execution_command()],
        };

        assert_roundtrips::<
            bsmr_test_proto::PrepareForLocalExecutionResponse,
            PrepareForLocalExecutionResult,
        >(&result);
    }

    #[test]
    fn test_executable_roundtrip() {
        let declared_output = DeclaredOutput {
            name: OutputName::unchecked_new("name".to_owned()),
            remote_storage_config: RemoteStorageConfig::new(false),
        };

        let test_executable = TestExecutable {
            stage: TestStage::Listing {
                suite: "name".to_owned(),
                cacheable: true,
            },
            target: ConfiguredTargetHandle(42),
            cmd: vec![
                ArgValue {
                    content: ArgValueContent::ExternalRunnerSpecValue(
                        ExternalRunnerSpecValue::Verbatim("arg".to_owned()),
                    ),
                    format: None,
                },
                ArgValue {
                    content: ArgValueContent::DeclaredOutput(declared_output.name.clone()),
                    format: Some("--output={}".to_owned()),
                },
            ],
            env: [(
                "FOO".to_owned(),
                ArgValue {
                    content: ArgValueContent::ExternalRunnerSpecValue(
                        ExternalRunnerSpecValue::EnvHandle(EnvHandle("FOO".to_owned())),
                    ),
                    format: None,
                },
            )]
            .into_iter()
            .collect(),
            pre_create_dirs: vec![declared_output],
        };

        assert_roundtrips::<bsmr_test_proto::TestExecutable, TestExecutable>(&test_executable);
    }
}
