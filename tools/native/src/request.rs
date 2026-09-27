//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Validate the shared action protocol before the native worker acquires an identity.

use std::fs::File;
use std::io;
use std::os::unix::fs::FileExt;
use std::path::PathBuf;
use std::time::Duration;

use bsmr_sandbox::{GuestAction, MAX_ACTION_BYTES, MAX_TIMEOUT_MS, PROTOCOL_VERSION};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::archive;

/// Untrusted wire data. The worker accepts it only through `Request::read`.
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Wire {
    /// Installed runtime identity supplied by the unprivileged executor.
    pub environment: String,
    /// SHA-256 of the complete input archive passed on the second descriptor.
    pub input: String,
    /// Existing BSMR action schema, shared with the other execution backends.
    pub action: GuestAction,
}

/// A bounded action bound to the administrator's installed runtime.
pub struct Request(Wire);

/// Refusals happen before creating a root or starting a child.
#[derive(Debug, Error)]
pub enum Error {
    #[error("native action exceeds {MAX_ACTION_BYTES} bytes")]
    Size,
    #[error("native action does not match the installed runtime")]
    Environment,
    #[error("native input identity must be a lowercase SHA-256 digest")]
    Digest,
    #[error("native action protocol must be {PROTOCOL_VERSION}, got {0}")]
    Protocol(u32),
    #[error("native action has an empty command or an invalid argument or environment key")]
    Command,
    #[error("native action path is invalid or overlaps another output: {0:?}")]
    Path(PathBuf),
    #[error("native action timeout exceeds {MAX_TIMEOUT_MS} milliseconds")]
    Timeout,
    #[error("native request I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("native request encoding is invalid: {0}")]
    Encoding(#[from] serde_json::Error),
}

impl Request {
    /// Read without using the sender's shared file position, then validate every boundary.
    pub fn read(file: &File, environment: &str) -> Result<Self, Error> {
        let size = file.metadata()?.len();
        if size > MAX_ACTION_BYTES {
            return Err(Error::Size);
        }
        let mut bytes = vec![0; usize::try_from(size).map_err(|_| Error::Size)?];
        file.read_exact_at(&mut bytes, 0)?;
        let wire: Wire = serde_json::from_slice(&bytes)?;
        if wire.environment != environment {
            return Err(Error::Environment);
        }
        if wire.input.len() != 64
            || !wire
                .input
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err(Error::Digest);
        }
        validate(&wire.action)?;
        Ok(Self(wire))
    }

    /// Return the validated action without permitting mutation of its fields.
    #[must_use]
    pub fn action(&self) -> &GuestAction {
        &self.0.action
    }

    /// Return the content identity that archive capture must verify.
    #[must_use]
    pub fn input(&self) -> &str {
        &self.0.input
    }

    /// Every action has a finite deadline, even when its caller omitted one.
    #[must_use]
    pub fn timeout(&self) -> Duration {
        Duration::from_millis(self.0.action.timeout_ms.unwrap_or(MAX_TIMEOUT_MS))
    }
}

/// Keep command validation independent of transport and temporary filesystem paths.
fn validate(action: &GuestAction) -> Result<(), Error> {
    if action.protocol != PROTOCOL_VERSION {
        return Err(Error::Protocol(action.protocol));
    }
    if action.arguments.first().is_none_or(String::is_empty)
        || action
            .arguments
            .iter()
            .any(|argument| argument.contains('\0'))
        || action
            .environment
            .iter()
            .any(|(key, value)| key.is_empty() || key.contains(['=', '\0']) || value.contains('\0'))
    {
        return Err(Error::Command);
    }
    if !action.working_directory.as_os_str().is_empty()
        && !archive::normal(&action.working_directory)
    {
        return Err(Error::Path(action.working_directory.clone()));
    }
    let mut outputs: Vec<_> = action.outputs.iter().map(|output| &output.path).collect();
    outputs.sort();
    for (index, path) in outputs.iter().enumerate() {
        if !archive::normal(path)
            || outputs[..index]
                .iter()
                .any(|parent| path.starts_with(parent))
        {
            return Err(Error::Path((*path).clone()));
        }
    }
    if action
        .timeout_ms
        .is_some_and(|value| value > MAX_TIMEOUT_MS)
    {
        return Err(Error::Timeout);
    }
    Ok(())
}
