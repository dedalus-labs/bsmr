//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

//! Query the current loader's shared-cache identity without guessing host paths.

use std::ffi::CStr;
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::PathBuf;

use anyhow::Result;
use anyhow::ensure;

unsafe extern "C" {
    fn dyld_shared_cache_file_path() -> *const std::ffi::c_char;
}

/// Resolve the exact OS cache backing this process through dyld's native API.
pub(crate) fn cache() -> Result<PathBuf> {
    // SAFETY: dyld returns a process-lifetime C string or null. No ownership transfers.
    let path = unsafe { dyld_shared_cache_file_path() };
    ensure!(!path.is_null(), "dyld did not report an OS shared cache");
    // SAFETY: the checked non-null pointer names dyld's immutable, NUL-terminated path.
    let path = unsafe { CStr::from_ptr(path) };
    Ok(PathBuf::from(OsStr::from_bytes(path.to_bytes())))
}

#[cfg(test)]
mod tests {
    /// The native API must name actual bytes, including on hosts using OS cryptexes.
    #[test]
    fn invariant_loaded_cache_has_a_backing_file() {
        let path = super::cache().unwrap();
        assert!(path.is_absolute());
        assert!(path.metadata().unwrap().is_file());
    }
}
