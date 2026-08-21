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

use bsmr_common::convert::ProstDurationExt;
use bsmr_error::BsmrErrorContext;
use bsmr_error::internal_error;
use bsmr_execute_local::CommandEvent;
use bsmr_execute_local::GatherOutputStatus;
use bsmr_resource_control::OrphanProcessInfo;
use futures::stream::Stream;
use futures::stream::StreamExt;

pub(crate) fn encode_event_stream<S>(
    s: S,
) -> impl Stream<Item = Result<bsmr_forkserver_proto::CommandEvent, tonic::Status>>
where
    S: Stream<Item = bsmr_error::Result<CommandEvent>>,
{
    fn convert_event(e: CommandEvent) -> bsmr_forkserver_proto::CommandEvent {
        use bsmr_forkserver_proto::command_event::Data;

        let (data, orphans) = match e {
            CommandEvent::Stdout(bytes) => (
                Data::Stdout(bsmr_forkserver_proto::StreamEvent {
                    data: bytes.to_vec(),
                }),
                Vec::new(),
            ),
            CommandEvent::Stderr(bytes) => (
                Data::Stderr(bsmr_forkserver_proto::StreamEvent {
                    data: bytes.to_vec(),
                }),
                Vec::new(),
            ),
            CommandEvent::Exit(
                GatherOutputStatus::Finished {
                    exit_code,
                    execution_stats,
                },
                orphans,
            ) => (
                Data::Exit(bsmr_forkserver_proto::ExitEvent {
                    exit_code,
                    execution_stats: execution_stats.map(|s| {
                        bsmr_forkserver_proto::CollectedExecutionStats {
                            cpu_instructions_user: s.cpu_instructions_user,
                            cpu_instructions_kernel: s.cpu_instructions_kernel,
                            userspace_events: s.userspace_events,
                            kernel_events: s.kernel_events,
                        }
                    }),
                }),
                orphans,
            ),
            CommandEvent::Exit(GatherOutputStatus::TimedOut(duration), orphans) => (
                Data::Timeout(bsmr_forkserver_proto::TimeoutEvent {
                    duration: duration.try_into().ok(),
                }),
                orphans,
            ),
            CommandEvent::Exit(GatherOutputStatus::Cancelled, orphans) => {
                (Data::Cancel(bsmr_forkserver_proto::CancelEvent {}), orphans)
            }
            CommandEvent::Exit(GatherOutputStatus::SpawnFailed(reason), orphans) => (
                Data::SpawnFailed(bsmr_forkserver_proto::SpawnFailedEvent { reason }),
                orphans,
            ),
        };

        bsmr_forkserver_proto::CommandEvent {
            data: Some(data),
            orphan_processes: orphans
                .into_iter()
                .map(|o| bsmr_forkserver_proto::OrphanProcess {
                    pid: o.pid,
                    comm: o.comm,
                })
                .collect(),
        }
    }

    fn convert_err(e: bsmr_error::Error) -> tonic::Status {
        tonic::Status::unknown(format!("{e:#}"))
    }

    s.map(|r| r.map(convert_event).map_err(convert_err))
}

pub(crate) fn decode_event_stream<S>(s: S) -> impl Stream<Item = bsmr_error::Result<CommandEvent>>
where
    S: Stream<Item = Result<bsmr_forkserver_proto::CommandEvent, tonic::Status>>,
{
    fn convert_event(e: bsmr_forkserver_proto::CommandEvent) -> bsmr_error::Result<CommandEvent> {
        use bsmr_forkserver_proto::command_event::Data;

        let orphans: Vec<OrphanProcessInfo> = e
            .orphan_processes
            .into_iter()
            .map(|o| OrphanProcessInfo {
                pid: o.pid,
                comm: o.comm,
            })
            .collect();

        let event = match e.data.ok_or_else(|| internal_error!("Missing `data`"))? {
            Data::Stdout(bsmr_forkserver_proto::StreamEvent { data }) => {
                CommandEvent::Stdout(data.into())
            }
            Data::Stderr(bsmr_forkserver_proto::StreamEvent { data }) => {
                CommandEvent::Stderr(data.into())
            }
            Data::Exit(bsmr_forkserver_proto::ExitEvent {
                exit_code,
                execution_stats,
            }) => CommandEvent::Exit(
                GatherOutputStatus::Finished {
                    exit_code,
                    execution_stats: execution_stats.map(|s| {
                        bsmr_execute_local::CollectedExecutionStats {
                            cpu_instructions_user: s.cpu_instructions_user,
                            cpu_instructions_kernel: s.cpu_instructions_kernel,
                            userspace_events: s.userspace_events,
                            kernel_events: s.kernel_events,
                        }
                    }),
                },
                orphans,
            ),
            Data::Timeout(bsmr_forkserver_proto::TimeoutEvent { duration }) => CommandEvent::Exit(
                GatherOutputStatus::TimedOut(
                    duration
                        .ok_or_else(|| internal_error!("Missing `duration`"))?
                        .try_into_duration()
                        .bsmr_error_context("Invalid `duration`")?,
                ),
                orphans,
            ),
            Data::Cancel(bsmr_forkserver_proto::CancelEvent {}) => {
                CommandEvent::Exit(GatherOutputStatus::Cancelled, orphans)
            }
            Data::SpawnFailed(bsmr_forkserver_proto::SpawnFailedEvent { reason }) => {
                CommandEvent::Exit(GatherOutputStatus::SpawnFailed(reason), orphans)
            }
        };

        Ok(event)
    }

    fn convert_err(e: tonic::Status) -> bsmr_error::Error {
        bsmr_error::bsmr_error!(
            bsmr_error::ErrorTag::Tier0,
            "forkserver error: {}",
            e.message()
        )
    }

    s.map(|r| r.map_err(convert_err).and_then(convert_event))
}
