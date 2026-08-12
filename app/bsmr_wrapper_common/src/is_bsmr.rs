//===----------------------------------------------------------------------===//
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

use std::env;
use std::ffi::OsStr;
use std::path::Path;
use std::path::PathBuf;
use std::sync::OnceLock;

use dupe::Dupe;

#[derive(Copy, Clone, Dupe)]
pub enum WhoIsAsking {
    Bessemer,
    BuckWrapper,
}

pub(crate) fn is_bsmr_exe(path: &Path, who_is_asking: WhoIsAsking) -> bool {
    let Some(file_stem) = path.file_stem() else {
        return false;
    };
    // On linux when the running executable is deleted or unlinked the string ' (deleted)' is appended to symlinked file in /proc/<pid>/exe
    if [
        OsStr::new("bsmr"),
        OsStr::new("bsmr (deleted)"),
        OsStr::new("bsmr-daemon"),
        OsStr::new("bsmr-daemon (deleted)"),
        OsStr::new("bsmr"),
        OsStr::new("bsmr (deleted)"),
        OsStr::new("bsmr-daemon"),
        OsStr::new("bsmr-daemon (deleted)"),
    ]
    .contains(&file_stem)
    {
        true
    } else {
        match who_is_asking {
            WhoIsAsking::BuckWrapper => {
                // We don't know another name of the bsmr executable in the wrapper.
                false
            }
            WhoIsAsking::Bessemer => {
                static CURRENT_EXE: OnceLock<PathBuf> = OnceLock::new();
                if let Ok(current_exe) = CURRENT_EXE.get_or_try_init(env::current_exe) {
                    if let Some(current_exe_file_stem) = current_exe.file_stem() {
                        if current_exe_file_stem == file_stem {
                            return true;
                        }
                    }
                }
                false
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::path::Path;

    use crate::is_bsmr::WhoIsAsking;
    use crate::is_bsmr::is_bsmr_exe;

    #[test]
    fn test_is_bsmr_exe() {
        let (fake_bsmr, other_path) = if cfg!(windows) {
            ("C:\\dir\\bsmr.exe", "C:\\dir\\other.exe")
        } else {
            ("/dir/bsmr", "/dir/other")
        };

        assert!(is_bsmr_exe(Path::new(fake_bsmr), WhoIsAsking::Bessemer));
        assert!(is_bsmr_exe(Path::new(fake_bsmr), WhoIsAsking::BuckWrapper));

        let current_exe = env::current_exe().unwrap();

        assert!(is_bsmr_exe(&current_exe, WhoIsAsking::Bessemer));
        assert!(!is_bsmr_exe(&current_exe, WhoIsAsking::BuckWrapper));

        assert!(!is_bsmr_exe(Path::new(other_path), WhoIsAsking::Bessemer));
        assert!(!is_bsmr_exe(
            Path::new(other_path),
            WhoIsAsking::BuckWrapper
        ));
    }
}
