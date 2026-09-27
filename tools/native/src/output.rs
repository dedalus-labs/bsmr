//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Serialize only completed native actions into the existing BSMR result archive.

use std::fs;
use std::fs::File;
use std::io::{self, Seek, SeekFrom, Write};
use std::os::unix::process::ExitStatusExt;
use std::path::{Path, PathBuf};

use bsmr_sandbox::{
    GuestResultEnvelope, MAX_OUTPUT_ARCHIVE_BYTES, MAX_STREAM_BYTES, PROTOCOL_VERSION,
};
use thiserror::Error;

use crate::job::Completed;
use crate::run::Outcome;

/// Refused output never becomes a published action result.
#[derive(Debug, Error)]
pub enum Error {
    #[error("native output stream {name} has {bytes} bytes, limit {MAX_STREAM_BYTES}")]
    Stream { name: &'static str, bytes: u64 },
    #[error("native output contains an unsupported file type: {0:?}")]
    Type(PathBuf),
    #[error("native child status contains neither an exit code nor a signal")]
    Status,
    #[error("native result archive exceeds {0} bytes")]
    Limit(u64),
    #[error("native result encoding failed: {0}")]
    Encoding(#[from] serde_json::Error),
    #[error("native output I/O failed: {0}")]
    Io(#[from] io::Error),
}

/// Cap the entire archive, including metadata, before the caller can accept it.
struct Bounded<const LIMIT: u64>(File, u64);

impl<const LIMIT: u64> Write for Bounded<LIMIT> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() as u64 > LIMIT.saturating_sub(self.1) {
            return Err(io::Error::other(Error::Limit(LIMIT)));
        }
        let written = self.0.write(bytes)?;
        self.1 += written as u64;
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.0.flush()
    }
}

/// Write the shared result format after cleanup, preserving the actual exit reason.
pub fn write(completed: &mut Completed) -> Result<(), Error> {
    let (exit_code, timed_out) = match completed.outcome() {
        Outcome::Exited(status) => (
            status
                .code()
                .or_else(|| status.signal().map(|signal| 128 + signal))
                .ok_or(Error::Status)?,
            false,
        ),
        Outcome::TimedOut => (124, true),
        Outcome::Cancelled => (130, false),
    };
    let mut output = completed.archive().try_clone()?;
    output.set_len(0)?;
    output.seek(SeekFrom::Start(0))?;
    let mut archive = tar::Builder::new(Bounded::<MAX_OUTPUT_ARCHIVE_BYTES>(output, 0));
    archive.follow_symlinks(false);
    let result = serde_json::to_vec(&GuestResultEnvelope {
        protocol: PROTOCOL_VERSION,
        exit_code,
        timed_out,
    })?;
    let mut header = tar::Header::new_gnu();
    header.set_mode(0o644);
    header.set_size(result.len() as u64);
    header.set_cksum();
    archive.append_data(&mut header, ".bsmr/result.json", result.as_slice())?;
    let (stdout, stderr) = completed.streams()?;
    for (name, mut file) in [(".bsmr/stdout", stdout), (".bsmr/stderr", stderr)] {
        let bytes = file.metadata()?.len();
        if bytes > MAX_STREAM_BYTES {
            return Err(Error::Stream { name, bytes });
        }
        archive.append_file(name, &mut file)?;
    }
    for (declaration, source) in completed.outputs() {
        match fs::symlink_metadata(&source) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            result => {
                result?;
            }
        }
        append(
            &mut archive,
            &source,
            &Path::new("outputs").join(&declaration.path),
        )?;
    }
    archive.finish()?;
    Ok(())
}

/// Stream supported entries once, without following links or opening special files.
fn append<W: Write>(
    archive: &mut tar::Builder<W>,
    source: &Path,
    destination: &Path,
) -> Result<(), Error> {
    let metadata = fs::symlink_metadata(source)?;
    if metadata.is_dir() {
        archive.append_dir(destination, source)?;
        for entry in fs::read_dir(source)? {
            let entry = entry?;
            append(archive, &entry.path(), &destination.join(entry.file_name()))?;
        }
    } else if metadata.is_symlink() {
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Symlink);
        header.set_mode(0o777);
        header.set_size(0);
        archive.append_link(&mut header, destination, fs::read_link(source)?)?;
    } else if metadata.is_file() {
        archive.append_path_with_name(source, destination)?;
    } else {
        return Err(Error::Type(source.to_owned()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn output_budget_includes_every_written_byte() {
        let mut output = Bounded::<1>(tempfile::tempfile().unwrap(), 0);
        output.write_all(b"x").unwrap();
        assert!(output.write_all(b"x").is_err());
    }

    #[test]
    fn collection_rejects_devices_without_reading_them() {
        assert!(matches!(
            append(
                &mut tar::Builder::new(Vec::new()),
                Path::new("/dev/null"),
                Path::new("output")
            ),
            Err(Error::Type(_))
        ));
    }

    #[test]
    fn output_links_are_preserved_without_opening_their_targets() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("link");
        std::os::unix::fs::symlink("/dev/null", &source).unwrap();
        let mut archive = tar::Builder::new(Vec::new());
        append(&mut archive, &source, Path::new("output")).unwrap();
        let bytes = archive.into_inner().unwrap();
        let mut archive = tar::Archive::new(bytes.as_slice());
        let entry = archive.entries().unwrap().next().unwrap().unwrap();
        assert!(entry.header().entry_type().is_symlink());
        assert_eq!(entry.link_name().unwrap().unwrap(), Path::new("/dev/null"));
    }
}
