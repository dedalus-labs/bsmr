//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Implements the bounded local HTTP client used by trusted Firecracker tools.

use std::io::ErrorKind;
use std::io::Read;
use std::io::Write;
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::path::PathBuf;
use std::time::Duration;

use serde::Serialize;
use thiserror::Error;

const MAX_RESPONSE_BYTES: usize = 64 * 1024;

/// A typed failure from Firecracker's local control plane.
#[derive(Debug, Error)]
pub enum ApiError {
    #[error("Firecracker API I/O failure at {socket:?}: {source}")]
    Io {
        socket: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to serialize Firecracker API request: {0}")]
    Serialize(#[from] serde_json::Error),
    #[error("Firecracker API request {path} failed: {response}")]
    Response { path: String, response: String },
}

/// A bounded client for one Firecracker API socket.
pub struct ApiClient<'a> {
    socket: &'a Path,
    timeout: Duration,
}

impl<'a> ApiClient<'a> {
    /// Binds a client to one local socket and I/O deadline.
    #[must_use]
    pub fn new(socket: &'a Path, timeout: Duration) -> Self {
        Self { socket, timeout }
    }

    /// Sends one JSON PUT and accepts only a successful HTTP response.
    pub fn put<T: Serialize>(&self, path: &str, value: &T) -> Result<(), ApiError> {
        self.request("PUT", path, value)
    }

    /// Sends one JSON PATCH and accepts only a successful HTTP response.
    pub fn patch<T: Serialize>(&self, path: &str, value: &T) -> Result<(), ApiError> {
        self.request("PATCH", path, value)
    }

    /// Sends one bounded JSON request over the local control socket.
    fn request<T: Serialize>(
        &self,
        method: &'static str,
        path: &str,
        value: &T,
    ) -> Result<(), ApiError> {
        let body = serde_json::to_vec(value)?;
        let mut stream = UnixStream::connect(self.socket).map_err(|source| self.io(source))?;
        stream
            .set_read_timeout(Some(self.timeout))
            .and_then(|()| stream.set_write_timeout(Some(self.timeout)))
            .map_err(|source| self.io(source))?;
        write!(
            stream,
            "{method} {path} HTTP/1.1\r\nHost: localhost\r\nAccept: application/json\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .and_then(|_| stream.write_all(&body))
        .map_err(|source| self.io(source))?;
        let response = read_response(&mut stream).map_err(|source| self.io(source))?;
        if !response
            .lines()
            .next()
            .is_some_and(|status| status.starts_with("HTTP/1.1 2"))
        {
            return Err(ApiError::Response {
                path: path.to_owned(),
                response,
            });
        }
        Ok(())
    }

    /// Attaches the configured socket identity to one I/O error.
    fn io(&self, source: std::io::Error) -> ApiError {
        ApiError::Io {
            socket: self.socket.to_owned(),
            source,
        }
    }
}

/// Reads one bounded HTTP/1.1 response using explicit message framing.
fn read_response(stream: &mut UnixStream) -> std::io::Result<String> {
    let mut response = Vec::new();
    let (header_length, content_length) = read_headers(stream, &mut response)?;
    let message_length = header_length
        .checked_add(content_length)
        .filter(|length| *length <= MAX_RESPONSE_BYTES)
        .ok_or_else(|| invalid("Firecracker API response is too large"))?;
    if response.len() > message_length {
        return Err(invalid("Firecracker API response exceeds Content-Length"));
    }
    let received = response.len();
    response.resize(message_length, 0);
    stream.read_exact(&mut response[received..])?;
    String::from_utf8(response).map_err(|error| std::io::Error::new(ErrorKind::InvalidData, error))
}

/// Reads and validates bounded HTTP headers before the response body.
fn read_headers(
    stream: &mut UnixStream,
    response: &mut Vec<u8>,
) -> std::io::Result<(usize, usize)> {
    loop {
        let mut headers = [httparse::EMPTY_HEADER; 16];
        let mut parsed = httparse::Response::new(&mut headers);
        if let httparse::Status::Complete(length) = parsed
            .parse(response)
            .map_err(|error| std::io::Error::new(ErrorKind::InvalidData, error))?
        {
            return Ok((length, content_length(&parsed)?));
        }
        let remaining = MAX_RESPONSE_BYTES - response.len();
        if remaining == 0 {
            return Err(invalid("Firecracker API headers are too large"));
        }
        let mut chunk = [0u8; 4096];
        let limit = remaining.min(chunk.len());
        let read = stream.read(&mut chunk[..limit])?;
        if read == 0 {
            return Err(std::io::Error::new(
                ErrorKind::UnexpectedEof,
                "Firecracker API response ended before its headers",
            ));
        }
        response.extend_from_slice(&chunk[..read]);
    }
}

/// Returns the one valid response body length for a parsed status.
fn content_length(response: &httparse::Response<'_, '_>) -> std::io::Result<usize> {
    if response.version != Some(1) {
        return Err(invalid("Firecracker API response is not HTTP/1.1"));
    }
    let status = response
        .code
        .ok_or_else(|| invalid("Firecracker API response has no status"))?;
    let lengths = response
        .headers
        .iter()
        .filter(|header| header.name.eq_ignore_ascii_case("content-length"))
        .map(|header| {
            std::str::from_utf8(header.value)
                .map_err(|error| std::io::Error::new(ErrorKind::InvalidData, error))?
                .parse::<usize>()
                .map_err(|error| std::io::Error::new(ErrorKind::InvalidData, error))
        })
        .collect::<std::io::Result<Vec<_>>>()?;
    match (status, lengths.as_slice()) {
        (204, []) => Ok(0),
        (_, [length]) => Ok(*length),
        _ => Err(invalid(
            "Firecracker API response has invalid Content-Length framing",
        )),
    }
}

/// Constructs one invalid-data error from a static protocol invariant.
fn invalid(message: &'static str) -> std::io::Error {
    std::io::Error::new(ErrorKind::InvalidData, message)
}

#[cfg(test)]
mod tests {
    use std::io::Write;
    use std::os::unix::net::UnixStream;

    use super::read_response;

    /// The API client reads the declared body but not a second response.
    #[test]
    fn response_framing_is_exact() {
        let (mut writer, mut reader) = UnixStream::pair().unwrap();
        writer
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}extra")
            .unwrap();
        assert!(read_response(&mut reader).is_err());
    }

    /// A bodyless Firecracker success is valid without Content-Length.
    #[test]
    fn no_content_response_is_valid() {
        let (mut writer, mut reader) = UnixStream::pair().unwrap();
        writer
            .write_all(b"HTTP/1.1 204 No Content\r\n\r\n")
            .unwrap();
        assert_eq!(
            read_response(&mut reader).unwrap(),
            "HTTP/1.1 204 No Content\r\n\r\n"
        );
    }
}
