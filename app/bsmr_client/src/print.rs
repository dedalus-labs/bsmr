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

use std::io::Write;
use std::path::PathBuf;

use bsmr_client_ctx::common::PrintOutputsFormat;
use bsmr_client_ctx::exit_result::ClientIoError;

pub struct PrintOutputs<W> {
    out: W,
    root_path: Option<PathBuf>,
    format: PrintOutputsFormat,
    empty: bool,
}

impl<W: Write> PrintOutputs<W> {
    pub fn new(
        mut out: W,
        root_path: Option<PathBuf>,
        format: PrintOutputsFormat,
    ) -> Result<Self, ClientIoError> {
        if format == PrintOutputsFormat::Json {
            write!(out, "{{")?;
        }
        Ok(PrintOutputs {
            out,
            root_path,
            format,
            empty: true,
        })
    }

    pub fn output(&mut self, target: &str, path: Option<&str>) -> Result<(), ClientIoError> {
        let windows_path;
        let absolute_path;
        let absolute_path_lossy;
        let path = if let Some(mut path) = path {
            if cfg!(windows) {
                windows_path = path.replace('/', "\\");
                path = &windows_path;
            }
            if let Some(root_path) = &self.root_path {
                absolute_path = root_path.join(path);
                absolute_path_lossy = absolute_path.to_string_lossy();
                &absolute_path_lossy
            } else {
                path
            }
        } else {
            ""
        };

        match self.format {
            PrintOutputsFormat::Plain => {
                writeln!(self.out, "{target} {path}")?;
            }
            PrintOutputsFormat::Simple => {
                writeln!(self.out, "{path}")?;
            }
            PrintOutputsFormat::Json => {
                if !self.empty {
                    write!(self.out, ",")?;
                }
                serde_json::to_writer(&mut self.out, target)?;
                write!(self.out, ":")?;
                serde_json::to_writer(&mut self.out, path)?;
                self.empty = false;
            }
        }

        Ok(())
    }

    pub fn finish(&mut self) -> Result<(), ClientIoError> {
        if self.format == PrintOutputsFormat::Json {
            writeln!(self.out, "}}")?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::str;

    use super::PrintOutputs;
    use super::PrintOutputsFormat;

    #[test]
    fn test() -> bsmr_error::Result<()> {
        for (format, root_path, expected) in [
            (
                PrintOutputsFormat::Plain,
                None,
                #[cfg(not(windows))]
                "\
                fb//third-party/rust:syn bsmr-out/third-party/rust/syn.rlib\n\
                fb//third-party/rust:serde_derive \n\
                ",
                #[cfg(windows)]
                "\
                fb//third-party/rust:syn bsmr-out\\third-party\\rust\\syn.rlib\n\
                fb//third-party/rust:serde_derive \n\
                ",
            ),
            (
                PrintOutputsFormat::Plain,
                Some(
                    #[cfg(not(windows))]
                    "/home/metaguest",
                    #[cfg(windows)]
                    "C:\\metaguest",
                ),
                #[cfg(not(windows))]
                "\
                fb//third-party/rust:syn /home/metaguest/bsmr-out/third-party/rust/syn.rlib\n\
                fb//third-party/rust:serde_derive \n\
                ",
                #[cfg(windows)]
                "\
                fb//third-party/rust:syn C:\\metaguest\\bsmr-out\\third-party\\rust\\syn.rlib\n\
                fb//third-party/rust:serde_derive \n\
                ",
            ),
            (
                PrintOutputsFormat::Simple,
                None,
                #[cfg(not(windows))]
                "\
                bsmr-out/third-party/rust/syn.rlib\n\
                \n\
                ",
                #[cfg(windows)]
                "\
                bsmr-out\\third-party\\rust\\syn.rlib\n\
                \n\
                ",
            ),
            (
                PrintOutputsFormat::Json,
                None,
                #[cfg(not(windows))]
                "\
                {\"fb//third-party/rust:syn\":\"bsmr-out/third-party/rust/syn.rlib\",\"fb//third-party/rust:serde_derive\":\"\"}\n\
                ",
                #[cfg(windows)]
                "\
                {\"fb//third-party/rust:syn\":\"bsmr-out\\\\third-party\\\\rust\\\\syn.rlib\",\"fb//third-party/rust:serde_derive\":\"\"}\n\
                ",
            ),
        ] {
            let mut out = Vec::new();
            let root_path = root_path.map(PathBuf::from);
            let mut print = PrintOutputs::new(&mut out, root_path, format)?;
            print.output(
                "fb//third-party/rust:syn",
                Some("bsmr-out/third-party/rust/syn.rlib"),
            )?;
            print.output("fb//third-party/rust:serde_derive", None)?;
            print.finish()?;
            assert_eq!(str::from_utf8(&out).unwrap(), expected);
        }

        Ok(())
    }

    #[test]
    fn test_json_empty() -> bsmr_error::Result<()> {
        let mut out = Vec::new();
        let mut print = PrintOutputs::new(&mut out, None, PrintOutputsFormat::Json)?;
        print.finish()?;
        assert_eq!(str::from_utf8(&out).unwrap(), "{}\n");
        Ok(())
    }
}
