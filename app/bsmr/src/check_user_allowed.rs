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

#[cfg(windows)]
pub(crate) fn check_user_allowed() -> bsmr_error::Result<()> {
    use core::ffi::c_void;
    use std::io;
    use std::mem;
    use std::mem::MaybeUninit;
    use std::ptr;

    use bsmr_core::ci::is_ci;
    use bsmr_error::BsmrErrorContext;
    use bsmr_wrapper_common::win::winapi_handle::WinapiHandle;
    use windows_sys::Win32::Foundation::HANDLE;
    use windows_sys::Win32::Security::GetTokenInformation;
    use windows_sys::Win32::Security::TOKEN_ELEVATION;
    use windows_sys::Win32::Security::TOKEN_QUERY;
    use windows_sys::Win32::Security::TokenElevation;
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    use windows_sys::Win32::System::Threading::OpenProcessToken;

    let mut handle: HANDLE = ptr::null_mut();
    let token_ok = unsafe { OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &mut handle) };
    if token_ok == 0 {
        return Err(io::Error::last_os_error()).bsmr_error_context("OpenProcessToken failed");
    }

    let handle = unsafe {
        WinapiHandle::new_check_last_os_error(handle).bsmr_error_context("OpenProcessToken")?
    };
    let size = mem::size_of::<TOKEN_ELEVATION>();
    let elevation: MaybeUninit<TOKEN_ELEVATION> = MaybeUninit::zeroed();
    let mut ret_size = 0;

    let success_get = unsafe {
        GetTokenInformation(
            handle.handle(),
            TokenElevation,
            elevation.as_ptr() as *mut c_void,
            size as u32,
            &mut ret_size,
        )
    };
    if success_get == 0 {
        return Err(io::Error::last_os_error()).bsmr_error_context("GetTokenInformation failed");
    }

    let elevation_struct: TOKEN_ELEVATION = unsafe { elevation.assume_init() };
    if elevation_struct.TokenIsElevated == 1 {
        // In CI, if bsmr got run from an admin shell, we need not worry that a
        // subsequent invocation might come from a non-admin shell. It almost
        // certainly will not.
        if !is_ci()? {
            tracing::warn!(
                "You're running bsmr from an admin shell. Invocations from non-admin shells will likely fail going forward. To remediate, run `bsmr clean` in this admin shell, then switch to a non-admin shell."
            );
        }
    }
    Ok(())
}

#[cfg(not(windows))]
pub(crate) fn check_user_allowed() -> bsmr_error::Result<()> {
    use std::os::unix::fs::MetadataExt;

    use bsmr_core::soft_error;
    use bsmr_error::internal_error;
    use bsmr_fs::error::IoResultExt;
    use bsmr_fs::fs_util;
    use bsmr_fs::paths::abs_path::AbsPath;

    #[derive(Debug, bsmr_error::Error)]
    #[error("bsmr is not allowed to run as root (unless home dir is owned by root)")]
    #[bsmr(tag = Input)]
    struct RootError;

    if nix::unistd::geteuid().is_root() {
        let home_dir = dirs::home_dir().ok_or_else(|| internal_error!("home dir not found"))?;
        if let Ok(home_dir) = AbsPath::new(&home_dir) {
            let home_dir_metadata = fs_util::metadata(home_dir).categorize_internal()?;
            if home_dir_metadata.uid() != 0 {
                soft_error!("root_not_allowed", RootError.into(), error_on_oss: true)?;
            }
        }
    }
    Ok(())
}
