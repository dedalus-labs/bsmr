//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Projects internal test results into Bessemer's stable build-event contract.

use std::time::SystemTime;

use bsmr_build_event_proto::BuildEvent;
use bsmr_build_event_proto::ExecutionKind;
use bsmr_build_event_proto::TestAttemptCompleted;
use bsmr_build_event_proto::TestId;
use bsmr_build_event_proto::TestOutcome;
use bsmr_build_event_proto::build_event::Payload;
use bsmr_common::convert::ProstDurationExt;
use bsmr_events::BuckEvent;

const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, bsmr_error::Error)]
enum BuildEventError {
    #[error("test result is missing `{field}`")]
    #[bsmr(tag = bsmr_error::ErrorTag::InvalidEvent)]
    Missing { field: &'static str },
    #[error("test result contains invalid `{field}` value `{value}`")]
    #[bsmr(tag = bsmr_error::ErrorTag::InvalidEvent)]
    InvalidEnum { field: &'static str, value: i32 },
    #[error("test result status `{status}` is not a completed test attempt")]
    #[bsmr(tag = bsmr_error::ErrorTag::InvalidEvent)]
    InvalidOutcome { status: &'static str },
    #[error("build event timestamp precedes the Unix epoch")]
    #[bsmr(tag = bsmr_error::ErrorTag::InvalidEvent)]
    TimestampBeforeEpoch,
}

/// Projects one internal event when it represents a completed test attempt.
pub(super) fn project_test_result(
    event: &BuckEvent,
    sequence_number: u64,
) -> bsmr_error::Result<Option<BuildEvent>> {
    let result = match event.data() {
        bsmr_data::buck_event::Data::Instant(instant) => match instant.data.as_ref() {
            Some(bsmr_data::instant_event::Data::TestResult(result)) => result,
            _ => return Ok(None),
        },
        _ => return Ok(None),
    };
    let Some(outcome) = test_outcome(result.status)? else {
        return Ok(None);
    };
    let attempt = result.attempt.as_ref().ok_or(BuildEventError::Missing {
        field: "attempt identity",
    })?;
    if attempt.attempt == 0 {
        return Err(BuildEventError::Missing {
            field: "positive attempt number",
        }
        .into());
    }
    let target = test_id(result, attempt)?;
    let duration = result
        .duration
        .as_ref()
        .ok_or(BuildEventError::Missing { field: "duration" })?
        .try_into_duration()?;
    let timestamp = event
        .timestamp()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_err(|_| BuildEventError::TimestampBeforeEpoch)?;
    Ok(Some(BuildEvent {
        schema_version: SCHEMA_VERSION,
        invocation_id: event.trace_id()?.to_string(),
        sequence_number,
        event_time_unix_millis: timestamp.as_millis().try_into()?,
        payload: Some(Payload::TestAttemptCompleted(TestAttemptCompleted {
            test: Some(target),
            action_digest: required_string(&attempt.action_digest, "action digest")?.to_owned(),
            attempt: attempt.attempt,
            outcome: outcome.into(),
            execution_kind: execution_kind(attempt.execution_kind)?.into(),
            duration_millis: duration.as_millis().try_into()?,
            message: result.msg.as_ref().map(|message| message.msg.clone()),
            details_digest: details_digest(&result.details)?,
            max_memory_used_bytes: result.max_memory_used_bytes,
        })),
    }))
}

/// Constructs a stable logical test identifier from native test metadata.
fn test_id(
    result: &bsmr_data::TestResult,
    attempt: &bsmr_data::TestAttempt,
) -> bsmr_error::Result<TestId> {
    let configured = result
        .target_label
        .as_ref()
        .ok_or(BuildEventError::Missing {
            field: "configured target",
        })?;
    let label = configured.label.as_ref().ok_or(BuildEventError::Missing {
        field: "target label",
    })?;
    let configuration = configured
        .configuration
        .as_ref()
        .ok_or(BuildEventError::Missing {
            field: "target configuration",
        })?;
    Ok(TestId {
        target: format!(
            "{}:{}",
            required_string(&label.package, "target package")?,
            required_string(&label.name, "target name")?
        ),
        configuration: required_string(&configuration.full_name, "target configuration")?
            .to_owned(),
        suite: required_string(&attempt.suite, "test suite")?.to_owned(),
        case: required_string(&result.name, "test case")?.to_owned(),
        variant: attempt.variant.clone(),
    })
}

/// Rejects empty values in fields that participate in stable identity.
fn required_string<'a>(value: &'a str, field: &'static str) -> bsmr_error::Result<&'a str> {
    if value.is_empty() {
        Err(BuildEventError::Missing { field }.into())
    } else {
        Ok(value)
    }
}

/// Maps factual test outcomes and ignores non-attempt lifecycle markers.
fn test_outcome(status: i32) -> bsmr_error::Result<Option<TestOutcome>> {
    let status =
        bsmr_data::TestStatus::try_from(status).map_err(|_| BuildEventError::InvalidEnum {
            field: "test status",
            value: status,
        })?;
    Ok(Some(match status {
        bsmr_data::TestStatus::Pass => TestOutcome::Pass,
        bsmr_data::TestStatus::Fail => TestOutcome::Fail,
        bsmr_data::TestStatus::Skip => TestOutcome::Skip,
        bsmr_data::TestStatus::Fatal => TestOutcome::Fatal,
        bsmr_data::TestStatus::Timeout => TestOutcome::Timeout,
        bsmr_data::TestStatus::Unknown => TestOutcome::Unknown,
        bsmr_data::TestStatus::InfraFailure => TestOutcome::InfraFailure,
        bsmr_data::TestStatus::ListingSuccess
        | bsmr_data::TestStatus::ListingFailed
        | bsmr_data::TestStatus::Omitted
        | bsmr_data::TestStatus::Rerun => {
            return Ok(None);
        }
        bsmr_data::TestStatus::NotSetTestStatus => {
            return Err(BuildEventError::InvalidOutcome { status: "not set" }.into());
        }
    }))
}

/// Maps the internal executor classification without losing cache provenance.
fn execution_kind(kind: i32) -> bsmr_error::Result<ExecutionKind> {
    let kind = bsmr_data::ActionExecutionKind::try_from(kind).map_err(|_| {
        BuildEventError::InvalidEnum {
            field: "execution kind",
            value: kind,
        }
    })?;
    Ok(match kind {
        bsmr_data::ActionExecutionKind::Local => ExecutionKind::Local,
        bsmr_data::ActionExecutionKind::Remote => ExecutionKind::Remote,
        bsmr_data::ActionExecutionKind::ActionCache => ExecutionKind::ActionCache,
        bsmr_data::ActionExecutionKind::Simple => ExecutionKind::Simple,
        bsmr_data::ActionExecutionKind::Deferred => ExecutionKind::Deferred,
        bsmr_data::ActionExecutionKind::LocalDepFile => ExecutionKind::LocalDepFile,
        bsmr_data::ActionExecutionKind::LocalWorker => ExecutionKind::LocalWorker,
        bsmr_data::ActionExecutionKind::RemoteDepFileCache => ExecutionKind::RemoteDepFileCache,
        bsmr_data::ActionExecutionKind::LocalActionCache => ExecutionKind::LocalActionCache,
        bsmr_data::ActionExecutionKind::RemoteWorker => ExecutionKind::RemoteWorker,
        bsmr_data::ActionExecutionKind::NotSet => {
            return Err(BuildEventError::InvalidEnum {
                field: "execution kind",
                value: kind.into(),
            }
            .into());
        }
    })
}

/// Hashes verbose output so consumers can group failures without ingesting logs.
fn details_digest(details: &str) -> bsmr_error::Result<Option<bsmr_build_event_proto::Digest>> {
    if details.is_empty() {
        return Ok(None);
    }
    Ok(Some(bsmr_build_event_proto::Digest {
        algorithm: "blake3".to_owned(),
        hash: blake3::hash(details.as_bytes()).to_hex().to_string(),
        size_bytes: details.len().try_into()?,
    }))
}

#[cfg(test)]
/// Builds a representative completed test-result event.
pub(super) fn test_result_event(
    trace_id: bsmr_wrapper_common::invocation_id::TraceId,
    attempt: Option<bsmr_data::TestAttempt>,
) -> std::sync::Arc<BuckEvent> {
    use std::time::Duration;

    std::sync::Arc::new(bsmr_events::BuckEvent::new(
        SystemTime::UNIX_EPOCH + Duration::from_millis(42),
        trace_id,
        None,
        None,
        bsmr_data::buck_event::Data::Instant(bsmr_data::InstantEvent {
            data: Some(
                bsmr_data::TestResult {
                    name: "case".to_owned(),
                    status: bsmr_data::TestStatus::Pass.into(),
                    duration: Some(prost_types::Duration {
                        seconds: 0,
                        nanos: 7_000_000,
                    }),
                    target_label: Some(bsmr_data::ConfiguredTargetLabel {
                        label: Some(bsmr_data::TargetLabel {
                            package: "cell//pkg".to_owned(),
                            name: "test".to_owned(),
                        }),
                        configuration: Some(bsmr_data::Configuration {
                            full_name: "cfg#abc".to_owned(),
                        }),
                        execution_configuration: None,
                    }),
                    attempt,
                    ..Default::default()
                }
                .into(),
            ),
        }),
    ))
}

#[cfg(test)]
/// Returns representative execution identity for projection tests.
pub(super) fn test_attempt() -> bsmr_data::TestAttempt {
    bsmr_data::TestAttempt {
        action_digest: "abcdef:123".to_owned(),
        suite: "unit".to_owned(),
        variant: Some("asan".to_owned()),
        attempt: 1,
        execution_kind: bsmr_data::ActionExecutionKind::Local.into(),
    }
}

#[cfg(test)]
mod tests {
    use bsmr_wrapper_common::invocation_id::TraceId;
    use dupe::Dupe;

    use super::*;

    /// Verifies that projection preserves all identity dimensions.
    #[test]
    fn invariant_test_observation_preserves_execution_identity() -> bsmr_error::Result<()> {
        let trace_id = TraceId::new();
        let event = test_result_event(trace_id.dupe(), Some(test_attempt()));

        let observation = project_test_result(&event, 3)?.expect("test observation");
        let Some(Payload::TestAttemptCompleted(attempt)) = observation.payload.as_ref() else {
            panic!("expected test-attempt payload");
        };

        assert_eq!(observation.schema_version, 1);
        assert_eq!(observation.invocation_id, trace_id.to_string());
        assert_eq!(observation.sequence_number, 3);
        assert_eq!(observation.event_time_unix_millis, 42);
        assert_eq!(attempt.action_digest, "abcdef:123");
        assert_eq!(attempt.attempt, 1);
        assert_eq!(attempt.test.as_ref().expect("test id").case, "case");
        Ok(())
    }

    /// Verifies that factual outcomes fail closed when identity is absent.
    #[test]
    fn invariant_completed_attempt_requires_execution_identity() {
        let event = test_result_event(TraceId::new(), None);

        let error = project_test_result(&event, 1).expect_err("identity must be required");

        assert!(error.to_string().contains("attempt identity"), "{error:#}");
    }
}
