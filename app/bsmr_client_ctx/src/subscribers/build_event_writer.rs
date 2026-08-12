//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Writes stable build observations without coupling the protocol to a backend.

use std::sync::Arc;

use async_trait::async_trait;
use bsmr_build_event_proto::BuildEvent;
use bsmr_events::BuckEvent;
use bsmr_fs::paths::abs_path::AbsPathBuf;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

use crate::subscribers::build_event::project_test_result;
use crate::subscribers::subscriber::EventSubscriber;

#[derive(Debug, bsmr_error::Error)]
enum BuildEventWriterError {
    #[error("build event sequence number overflowed")]
    #[bsmr(tag = bsmr_error::ErrorTag::InternalError)]
    SequenceOverflow,
    #[error("build event output file was not initialized")]
    #[bsmr(tag = bsmr_error::ErrorTag::InternalError)]
    OutputUnavailable,
}

/// Writes stable build observations as one JSON object per line.
pub(crate) struct BuildEventWriter {
    path: AbsPathBuf,
    file: Option<File>,
    next_sequence_number: u64,
}

impl BuildEventWriter {
    /// Creates a writer for `path` without opening it until event processing begins.
    pub(crate) fn new(path: AbsPathBuf) -> Self {
        Self {
            path,
            file: None,
            next_sequence_number: 1,
        }
    }

    /// Opens the destination exactly once, truncating any prior artifact.
    async fn ensure_file(&mut self) -> bsmr_error::Result<&mut File> {
        if self.file.is_none() {
            self.file = Some(File::create(&self.path).await?);
        }
        self.file
            .as_mut()
            .ok_or_else(|| BuildEventWriterError::OutputUnavailable.into())
    }

    /// Appends one complete JSON Lines record.
    async fn write(&mut self, observation: &BuildEvent) -> bsmr_error::Result<()> {
        let mut encoded = serde_json::to_vec(observation)?;
        encoded.push(b'\n');
        self.ensure_file().await?.write_all(&encoded).await?;
        Ok(())
    }
}

#[async_trait]
impl EventSubscriber for BuildEventWriter {
    fn name(&self) -> &'static str {
        "build event writer"
    }

    async fn handle_events(&mut self, events: &[Arc<BuckEvent>]) -> bsmr_error::Result<()> {
        self.ensure_file().await?;
        for event in events {
            if let Some(observation) = project_test_result(event, self.next_sequence_number)? {
                self.write(&observation).await?;
                self.next_sequence_number = self
                    .next_sequence_number
                    .checked_add(1)
                    .ok_or(BuildEventWriterError::SequenceOverflow)?;
            }
        }
        Ok(())
    }

    async fn finalize(mut self: Box<Self>) -> bsmr_error::Result<()> {
        if let Some(file) = &mut self.file {
            file.flush().await?;
            file.sync_all().await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use bsmr_fs::paths::abs_path::AbsPathBuf;
    use bsmr_wrapper_common::invocation_id::TraceId;
    use dupe::Dupe;

    use super::*;
    use crate::subscribers::build_event::test_attempt;
    use crate::subscribers::build_event::test_result_event;

    /// Verifies the serialized JSONL contract and monotonic sequence numbers.
    #[tokio::test]
    async fn invariant_writer_emits_stable_jsonl() -> bsmr_error::Result<()> {
        let directory = tempfile::tempdir()?;
        let path = AbsPathBuf::new(directory.path().join("build-events.jsonl"))?;
        let event = test_result_event(TraceId::new(), Some(test_attempt()));
        let mut writer = BuildEventWriter::new(path.clone());

        writer.handle_events(&[event.dupe(), event]).await?;
        Box::new(writer).finalize().await?;

        let output = tokio::fs::read_to_string(path).await?;
        let lines = output.lines().collect::<Vec<_>>();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].contains("\"outcome\":\"pass\""));
        assert!(lines[0].contains("\"execution_kind\":\"local\""));
        let first: BuildEvent = serde_json::from_str(lines[0])?;
        let second: BuildEvent = serde_json::from_str(lines[1])?;
        assert_eq!(first.sequence_number, 1);
        assert_eq!(second.sequence_number, 2);
        Ok(())
    }
}
