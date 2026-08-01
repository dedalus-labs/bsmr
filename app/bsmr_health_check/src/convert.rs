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

// gRPC to rust converters

use bsmr_error::BuckErrorContext;
use bsmr_error::internal_error;

use crate::interface::HealthCheckContextEvent;
use crate::interface::HealthCheckSnapshotData;
use crate::interface::HealthCheckType;
use crate::report::DisplayReport;
use crate::report::HealthIssue;
use crate::report::Message;
use crate::report::Remediation;
use crate::report::Report;
use crate::report::Severity;

impl TryFrom<i32> for Severity {
    type Error = bsmr_error::Error;
    fn try_from(s: i32) -> bsmr_error::Result<Self> {
        let severity = bsmr_health_check_proto::Severity::try_from(s)
            .buck_error_context("Invalid `severity`")?;
        Ok(match severity {
            bsmr_health_check_proto::Severity::Info => Severity::Info,
            bsmr_health_check_proto::Severity::Warning => Severity::Warning,
        })
    }
}

impl TryInto<i32> for Severity {
    type Error = bsmr_error::Error;
    fn try_into(self) -> bsmr_error::Result<i32> {
        Ok(match self {
            Severity::Info => bsmr_health_check_proto::Severity::Info,
            Severity::Warning => bsmr_health_check_proto::Severity::Warning,
        } as i32)
    }
}

impl TryFrom<bsmr_health_check_proto::Remediation> for Remediation {
    type Error = bsmr_error::Error;

    fn try_from(value: bsmr_health_check_proto::Remediation) -> bsmr_error::Result<Self> {
        Ok(
            match value
                .data
                .ok_or_else(|| internal_error!("Invalid `remediation`"))?
            {
                bsmr_health_check_proto::remediation::Data::Message(message) => {
                    Remediation::Message(message)
                }
                bsmr_health_check_proto::remediation::Data::Link(link) => Remediation::Link(link),
            },
        )
    }
}

impl TryInto<bsmr_health_check_proto::Remediation> for Remediation {
    type Error = bsmr_error::Error;

    fn try_into(self) -> bsmr_error::Result<bsmr_health_check_proto::Remediation> {
        let value = match self {
            Remediation::Message(message) => {
                bsmr_health_check_proto::remediation::Data::Message(message)
            }
            Remediation::Link(link) => bsmr_health_check_proto::remediation::Data::Link(link),
        };
        Ok(bsmr_health_check_proto::Remediation { data: Some(value) })
    }
}

impl TryFrom<i32> for HealthCheckType {
    type Error = bsmr_error::Error;

    fn try_from(value: i32) -> bsmr_error::Result<Self> {
        let value = bsmr_health_check_proto::HealthCheckType::try_from(value)
            .buck_error_context("Invalid `health_check_type`")?;
        Ok(match value {
            bsmr_health_check_proto::HealthCheckType::MemoryPressure => {
                HealthCheckType::MemoryPressure
            }
            bsmr_health_check_proto::HealthCheckType::LowDiskSpace => HealthCheckType::LowDiskSpace,
            bsmr_health_check_proto::HealthCheckType::SlowDownloadSpeed => {
                HealthCheckType::SlowDownloadSpeed
            }
            bsmr_health_check_proto::HealthCheckType::SlowBuild => HealthCheckType::SlowBuild,
            bsmr_health_check_proto::HealthCheckType::VpnEnabled => HealthCheckType::VpnEnabled,
            bsmr_health_check_proto::HealthCheckType::StableRevision => {
                HealthCheckType::StableRevision
            }
        })
    }
}

impl TryInto<i32> for HealthCheckType {
    type Error = bsmr_error::Error;

    fn try_into(self) -> bsmr_error::Result<i32> {
        Ok(match self {
            HealthCheckType::MemoryPressure => {
                bsmr_health_check_proto::HealthCheckType::MemoryPressure
            }
            HealthCheckType::LowDiskSpace => bsmr_health_check_proto::HealthCheckType::LowDiskSpace,
            HealthCheckType::SlowDownloadSpeed => {
                bsmr_health_check_proto::HealthCheckType::SlowDownloadSpeed
            }
            HealthCheckType::VpnEnabled => bsmr_health_check_proto::HealthCheckType::VpnEnabled,
            HealthCheckType::StableRevision => {
                bsmr_health_check_proto::HealthCheckType::StableRevision
            }
            HealthCheckType::SlowBuild => bsmr_health_check_proto::HealthCheckType::SlowBuild,
        } as i32)
    }
}

impl TryFrom<bsmr_health_check_proto::Message> for Message {
    type Error = bsmr_error::Error;

    fn try_from(value: bsmr_health_check_proto::Message) -> bsmr_error::Result<Self> {
        match value
            .data
            .ok_or_else(|| internal_error!("Invalid message format"))?
        {
            bsmr_health_check_proto::message::Data::Simple(text) => Ok(Message::Simple(text)),
            bsmr_health_check_proto::message::Data::Rich(rich_msg) => Ok(Message::Rich {
                header: rich_msg.header,
                body: rich_msg.body,
                footer: rich_msg.footer,
                compact: rich_msg.compact,
            }),
        }
    }
}

impl TryInto<bsmr_health_check_proto::Message> for Message {
    type Error = bsmr_error::Error;

    fn try_into(self) -> bsmr_error::Result<bsmr_health_check_proto::Message> {
        let data = match self {
            Message::Simple(text) => bsmr_health_check_proto::message::Data::Simple(text),
            Message::Rich {
                header,
                body,
                footer,
                compact,
            } => {
                bsmr_health_check_proto::message::Data::Rich(bsmr_health_check_proto::RichMessage {
                    header,
                    body,
                    footer,
                    compact,
                })
            }
        };
        Ok(bsmr_health_check_proto::Message { data: Some(data) })
    }
}

impl TryFrom<bsmr_health_check_proto::HealthIssue> for HealthIssue {
    type Error = bsmr_error::Error;

    fn try_from(value: bsmr_health_check_proto::HealthIssue) -> bsmr_error::Result<Self> {
        Ok(HealthIssue {
            severity: value.severity.try_into()?,
            message: value
                .message
                .ok_or_else(|| internal_error!("Missing message"))?
                .try_into()?,
            remediation: value.remediation.map(|r| r.try_into()).transpose()?,
        })
    }
}

impl TryInto<bsmr_health_check_proto::HealthIssue> for HealthIssue {
    type Error = bsmr_error::Error;

    fn try_into(self) -> bsmr_error::Result<bsmr_health_check_proto::HealthIssue> {
        Ok(bsmr_health_check_proto::HealthIssue {
            severity: self.severity.try_into()?,
            message: Some(self.message.try_into()?),
            remediation: self.remediation.map(|r| r.try_into()).transpose()?,
        })
    }
}

impl TryFrom<bsmr_health_check_proto::DisplayReport> for DisplayReport {
    type Error = bsmr_error::Error;

    fn try_from(value: bsmr_health_check_proto::DisplayReport) -> bsmr_error::Result<Self> {
        Ok(DisplayReport {
            health_check_type: value.health_check_type.try_into()?,
            health_issue: value.health_issue.map(|i| i.try_into()).transpose()?,
        })
    }
}
impl TryInto<bsmr_health_check_proto::DisplayReport> for DisplayReport {
    type Error = bsmr_error::Error;

    fn try_into(self) -> bsmr_error::Result<bsmr_health_check_proto::DisplayReport> {
        Ok(bsmr_health_check_proto::DisplayReport {
            health_check_type: self.health_check_type.try_into()?,
            health_issue: self.health_issue.map(|i| i.try_into()).transpose()?,
        })
    }
}

impl TryFrom<bsmr_health_check_proto::Report> for Report {
    type Error = bsmr_error::Error;

    fn try_from(value: bsmr_health_check_proto::Report) -> bsmr_error::Result<Self> {
        Ok(Report {
            display_report: value.display_report.map(|d| d.try_into()).transpose()?,
            tag: value.tag,
        })
    }
}

impl TryInto<bsmr_health_check_proto::Report> for Report {
    type Error = bsmr_error::Error;

    fn try_into(self) -> bsmr_error::Result<bsmr_health_check_proto::Report> {
        Ok(bsmr_health_check_proto::Report {
            display_report: self.display_report.map(|d| d.try_into()).transpose()?,
            tag: self.tag,
        })
    }
}

impl TryInto<bsmr_health_check_proto::HealthCheckContextEvent> for HealthCheckContextEvent {
    type Error = bsmr_error::Error;

    fn try_into(self) -> bsmr_error::Result<bsmr_health_check_proto::HealthCheckContextEvent> {
        Ok(match self {
            HealthCheckContextEvent::BranchedFromRevision(rev) => {
                bsmr_health_check_proto::HealthCheckContextEvent {
                    data: Some(bsmr_health_check_proto::health_check_context_event::Data::BranchedFromRevision(rev)),
                }
            }
            HealthCheckContextEvent::CommandStart(cmd) => {
                bsmr_health_check_proto::HealthCheckContextEvent {
                    data: Some(bsmr_health_check_proto::health_check_context_event::Data::CommandStart(cmd.clone())),
                }
            }
            HealthCheckContextEvent::ParsedTargetPatterns(patterns) => {
                bsmr_health_check_proto::HealthCheckContextEvent {
                    data: Some(bsmr_health_check_proto::health_check_context_event::Data::ParsedTargetPatterns(patterns.clone())),
                }
            }
            HealthCheckContextEvent::HasExcessCacheMisses() => {
                bsmr_health_check_proto::HealthCheckContextEvent {
                    data: Some(bsmr_health_check_proto::health_check_context_event::Data::HasExcessCacheMisses(true)),
                }
            }
            HealthCheckContextEvent::ExperimentConfigurations(system_info) => {
                bsmr_health_check_proto::HealthCheckContextEvent {
                    data: Some(bsmr_health_check_proto::health_check_context_event::Data::ExperimentConfigurations(system_info.clone())),
                }
            }
            HealthCheckContextEvent::TestSlowBuildThreshold(secs) => {
                bsmr_health_check_proto::HealthCheckContextEvent {
                    data: Some(bsmr_health_check_proto::health_check_context_event::Data::TestSlowBuildThresholdSecs(secs)),
                }
            }
        })
    }
}

impl TryFrom<bsmr_health_check_proto::HealthCheckContextEvent> for HealthCheckContextEvent {
    type Error = bsmr_error::Error;
    fn try_from(
        value: bsmr_health_check_proto::HealthCheckContextEvent,
    ) -> bsmr_error::Result<Self> {
        Ok( match value.data.ok_or_else(|| internal_error!("Invalid `health_check_context_event`"))? {
            bsmr_health_check_proto::health_check_context_event::Data::BranchedFromRevision(rev) => {
                HealthCheckContextEvent::BranchedFromRevision(rev)
            }
            bsmr_health_check_proto::health_check_context_event::Data::CommandStart(cmd) => {
                HealthCheckContextEvent::CommandStart(cmd)
            }
            bsmr_health_check_proto::health_check_context_event::Data::ParsedTargetPatterns(patterns) => {
                HealthCheckContextEvent::ParsedTargetPatterns(patterns)
            }
            bsmr_health_check_proto::health_check_context_event::Data::HasExcessCacheMisses(_) => {
                HealthCheckContextEvent::HasExcessCacheMisses()
            }
            bsmr_health_check_proto::health_check_context_event::Data::ExperimentConfigurations(system_info) => {
                HealthCheckContextEvent::ExperimentConfigurations(system_info)
            }
            bsmr_health_check_proto::health_check_context_event::Data::TestSlowBuildThresholdSecs(secs) => {
                HealthCheckContextEvent::TestSlowBuildThreshold(secs)
            }
        }
    )
    }
}

impl TryFrom<bsmr_health_check_proto::HealthCheckSnapshotData> for HealthCheckSnapshotData {
    type Error = bsmr_error::Error;

    fn try_from(
        value: bsmr_health_check_proto::HealthCheckSnapshotData,
    ) -> bsmr_error::Result<Self> {
        use std::time::Duration;
        use std::time::UNIX_EPOCH;

        let proto_timestamp = value.timestamp.ok_or_else(|| {
            bsmr_error::bsmr_error!(
                bsmr_error::ErrorTag::HealthCheck,
                "Missing timestamp in HealthCheckSnapshotData"
            )
        })?;

        // Convert protobuf Timestamp to SystemTime
        let duration = Duration::new(proto_timestamp.seconds as u64, proto_timestamp.nanos as u32);
        let timestamp = UNIX_EPOCH + duration;

        Ok(HealthCheckSnapshotData { timestamp })
    }
}

impl TryInto<bsmr_health_check_proto::HealthCheckSnapshotData> for HealthCheckSnapshotData {
    type Error = bsmr_error::Error;

    fn try_into(self) -> bsmr_error::Result<bsmr_health_check_proto::HealthCheckSnapshotData> {
        // Convert SystemTime to protobuf Timestamp
        let duration_since_epoch = self
            .timestamp
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|_e| {
                bsmr_error::bsmr_error!(
                    bsmr_error::ErrorTag::HealthCheck,
                    "Invalid timestamp in HealthCheckSnapshotData"
                )
            })?;

        let timestamp = Some(prost_types::Timestamp {
            seconds: duration_since_epoch.as_secs() as i64,
            nanos: duration_since_epoch.subsec_nanos() as i32,
        });

        Ok(bsmr_health_check_proto::HealthCheckSnapshotData { timestamp })
    }
}
