//===----------------------------------------------------------------------===//
// Copyright (c) 2026 Dedalus Labs, Inc. and its contributors
// SPDX-License-Identifier: Apache-2.0
//===----------------------------------------------------------------------===//

// Materializes a locked official Go SDK as repository-local build inputs.

//! Owns the repository-local lifecycle of a locked official Go SDK.
//!
//! Acquisition hard-links verified archive output into ignored SDK and tool
//! directories. A structured marker binds both directories to the committed lock and
//! is required before replacement, preventing generated cleanup from deleting user data.

use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process::Command;

use serde::Deserialize;
use serde::Serialize;

use super::GENERATED_BY;
use super::GoSdkArchive;
use super::GoToolchainError;
use super::GoToolchainLock;

const GO_BOOTSTRAP_WRAPPER_SOURCE: &str = include_str!("go_bootstrap_wrapper.go");
const SDK_DIRECTORY: &str = "toolchains/.bsmr-go-sdk";
const TOOLS_DIRECTORY: &str = "toolchains/.bsmr-go-tools";

/// Identity marker required on every frontend-owned acquisition directory.
#[derive(Debug, Deserialize, Serialize)]
struct AcquiredSdk {
    /// Proves directory ownership before replacement.
    generated_by: String,
    /// Distinguishes analysis placeholders from complete, usable acquisitions.
    state: String,
    /// Ties the installed compiler and tools to the locked Go semantics.
    version: String,
    /// Ties the installed bytes to the current execution operating system.
    os: String,
    /// Ties the installed bytes to the current execution architecture.
    arch: String,
    /// Ties the installed bytes to the authenticated official archive.
    sha256: String,
}

/// Creates only the owned source placeholders needed to analyze the acquisition target.
pub(crate) fn prepare_acquisition(
    root: &Path,
    lock: &GoToolchainLock,
) -> Result<(), GoToolchainError> {
    let archive = host_archive(lock)?;
    let metadata = AcquiredSdk {
        generated_by: GENERATED_BY.to_owned(),
        state: "acquiring".to_owned(),
        version: lock.version.clone(),
        os: archive.os.clone(),
        arch: archive.arch.clone(),
        sha256: archive.sha256.clone(),
    };
    for directory in [SDK_DIRECTORY, TOOLS_DIRECTORY] {
        let path = root.join(directory);
        if path.exists() {
            validate_owned_acquisition(&path)?;
            continue;
        }
        fs::create_dir(&path).map_err(|error| write_error(&path, error))?;
        write_acquisition_metadata(&path, &metadata)?;
    }
    let wrapper = root.join(TOOLS_DIRECTORY).join("go_wrapper");
    if !wrapper.exists() {
        fs::write(&wrapper, []).map_err(|error| write_error(&wrapper, error))?;
    }
    Ok(())
}

/// Installs one verified extracted SDK and bootstrap wrapper as repository-local inputs.
pub(crate) fn install_sdk(
    root: &Path,
    extracted: &Path,
    lock: &GoToolchainLock,
) -> Result<(), GoToolchainError> {
    let archive = host_archive(lock)?;
    verify_sdk_version(extracted, &lock.version)?;
    let toolchains = root.join("toolchains");
    let sdk_stage = tempfile::Builder::new()
        .prefix(".bsmr-go-sdk-stage-")
        .tempdir_in(&toolchains)
        .map_err(|error| write_error(&toolchains, error))?;
    copy_tree(extracted, sdk_stage.path())?;
    let metadata = AcquiredSdk {
        generated_by: GENERATED_BY.to_owned(),
        state: "acquired".to_owned(),
        version: lock.version.clone(),
        os: archive.os.clone(),
        arch: archive.arch.clone(),
        sha256: archive.sha256.clone(),
    };
    write_acquisition_metadata(sdk_stage.path(), &metadata)?;
    let tools_stage = tempfile::Builder::new()
        .prefix(".bsmr-go-tools-stage-")
        .tempdir_in(&toolchains)
        .map_err(|error| write_error(&toolchains, error))?;
    compile_bootstrap_wrapper(sdk_stage.path(), tools_stage.path(), archive)?;
    write_acquisition_metadata(tools_stage.path(), &metadata)?;
    replace_generated_directory(sdk_stage, &root.join(SDK_DIRECTORY))?;
    replace_generated_directory(tools_stage, &root.join(TOOLS_DIRECTORY))?;
    Ok(())
}

/// Returns the acquired SDK executable only when it matches the committed lock.
pub(crate) fn acquired_go(
    root: &Path,
    lock: &GoToolchainLock,
) -> Result<PathBuf, GoToolchainError> {
    let sdk = root.join(SDK_DIRECTORY);
    let tools = root.join(TOOLS_DIRECTORY);
    let archive = host_archive(lock)?;
    validate_acquired_directory(&sdk, lock, archive)?;
    validate_acquired_directory(&tools, lock, archive)?;
    verify_sdk_version(&sdk, &lock.version)?;
    let executable = sdk.join("bin/go");
    if !executable.is_file() || !tools.join("go_wrapper").is_file() {
        return Err(GoToolchainError::NotAcquired);
    }
    Ok(executable)
}

/// Validates one installed SDK component against the complete locked identity.
fn validate_acquired_directory(
    path: &Path,
    lock: &GoToolchainLock,
    archive: &GoSdkArchive,
) -> Result<(), GoToolchainError> {
    let metadata =
        fs::read(path.join(".bsmr-metadata.json")).map_err(|_| GoToolchainError::NotAcquired)?;
    let metadata = serde_json::from_slice::<AcquiredSdk>(&metadata)
        .map_err(|_| GoToolchainError::NotAcquired)?;
    if metadata.generated_by != GENERATED_BY
        || metadata.state != "acquired"
        || metadata.version != lock.version
        || metadata.os != archive.os
        || metadata.arch != archive.arch
        || metadata.sha256 != archive.sha256
    {
        return Err(GoToolchainError::NotAcquired);
    }
    Ok(())
}

/// Selects the official archive for the current execution host.
fn host_archive(lock: &GoToolchainLock) -> Result<&GoSdkArchive, GoToolchainError> {
    let os = match std::env::consts::OS {
        "macos" => "darwin",
        "linux" => "linux",
        other => {
            return Err(GoToolchainError::UnsupportedHost(
                other.to_owned(),
                std::env::consts::ARCH.to_owned(),
            ));
        }
    };
    let arch = match std::env::consts::ARCH {
        "aarch64" => "arm64",
        "x86_64" => "amd64",
        other => {
            return Err(GoToolchainError::UnsupportedHost(
                os.to_owned(),
                other.to_owned(),
            ));
        }
    };
    lock.archives
        .iter()
        .find(|archive| archive.os == os && archive.arch == arch)
        .ok_or_else(|| GoToolchainError::UnsupportedHost(os.to_owned(), arch.to_owned()))
}

/// Verifies the extracted SDK's own version marker.
fn verify_sdk_version(root: &Path, version: &str) -> Result<(), GoToolchainError> {
    let path = root.join("VERSION");
    let declared = fs::read_to_string(&path).map_err(|error| GoToolchainError::Read {
        path,
        message: error.to_string(),
    })?;
    let declared = declared.lines().next().unwrap_or_default();
    if declared == format!("go{version}") {
        Ok(())
    } else {
        Err(GoToolchainError::SdkVersion {
            locked: version.to_owned(),
            declared: declared.to_owned(),
        })
    }
}

/// Hard-links an extracted SDK without duplicating immutable archive bytes.
fn copy_tree(source: &Path, destination: &Path) -> Result<(), GoToolchainError> {
    for entry in walkdir::WalkDir::new(source)
        .min_depth(1)
        .follow_links(false)
    {
        let entry = entry.map_err(|error| GoToolchainError::Copy(error.to_string()))?;
        let relative = entry
            .path()
            .strip_prefix(source)
            .map_err(|error| GoToolchainError::Copy(error.to_string()))?;
        let target = destination.join(relative);
        if entry.file_type().is_dir() {
            fs::create_dir(&target).map_err(|error| write_error(&target, error))?;
        } else if entry.file_type().is_symlink() {
            let link = fs::read_link(entry.path()).map_err(|error| GoToolchainError::Read {
                path: entry.path().to_owned(),
                message: error.to_string(),
            })?;
            copy_symlink(&link, &target)?;
        } else if entry.file_type().is_file() {
            fs::hard_link(entry.path(), &target).map_err(|error| {
                GoToolchainError::Copy(format!(
                    "failed to hard-link `{}` to `{}`: {error}",
                    entry.path().display(),
                    target.display()
                ))
            })?;
        } else {
            return Err(GoToolchainError::Copy(format!(
                "unsupported SDK entry `{}`",
                entry.path().display()
            )));
        }
    }
    Ok(())
}

/// Recreates SDK symlinks on the supported Unix execution hosts.
#[cfg(unix)]
fn copy_symlink(source: &Path, destination: &Path) -> Result<(), GoToolchainError> {
    std::os::unix::fs::symlink(source, destination).map_err(|error| write_error(destination, error))
}

/// Keeps unsupported hosts buildable while failing acquisition explicitly.
#[cfg(not(unix))]
fn copy_symlink(_source: &Path, destination: &Path) -> Result<(), GoToolchainError> {
    Err(GoToolchainError::Copy(format!(
        "symbolic links are unsupported on host {} for `{}`",
        std::env::consts::OS,
        destination.display()
    )))
}

/// Builds the bootstrap wrapper with only the acquired SDK and explicit temporary state.
fn compile_bootstrap_wrapper(
    sdk: &Path,
    destination: &Path,
    archive: &GoSdkArchive,
) -> Result<(), GoToolchainError> {
    let source = destination.join("go_wrapper.go");
    let output = destination.join("go_wrapper");
    let cache = destination.join("cache");
    let temporary = destination.join("tmp");
    fs::write(&source, GO_BOOTSTRAP_WRAPPER_SOURCE).map_err(|error| write_error(&source, error))?;
    fs::create_dir(&cache).map_err(|error| write_error(&cache, error))?;
    fs::create_dir(&temporary).map_err(|error| write_error(&temporary, error))?;
    let result = Command::new(sdk.join("bin/go"))
        .args(["build", "-trimpath", "-buildvcs=false", "-o"])
        .arg(&output)
        .arg(&source)
        .env_clear()
        .env("CGO_ENABLED", "0")
        .env("GOARCH", &archive.arch)
        .env("GOCACHE", &cache)
        .env("GOENV", "off")
        .env("GOOS", &archive.os)
        .env("GOROOT", sdk)
        .env("GOTOOLCHAIN", "local")
        .env("TMPDIR", &temporary)
        .output()
        .map_err(|error| GoToolchainError::Wrapper(error.to_string()))?;
    if !result.status.success() {
        return Err(GoToolchainError::Wrapper(
            String::from_utf8_lossy(&result.stderr).trim().to_owned(),
        ));
    }
    fs::remove_file(&source).map_err(|error| write_error(&source, error))?;
    fs::remove_dir_all(&cache).map_err(|error| write_error(&cache, error))?;
    fs::remove_dir_all(&temporary).map_err(|error| write_error(&temporary, error))?;
    Ok(())
}

/// Records ownership and keeps generated payloads out of version control.
fn write_acquisition_metadata(root: &Path, metadata: &AcquiredSdk) -> Result<(), GoToolchainError> {
    let mut content = serde_json::to_string_pretty(metadata)
        .map_err(|error| GoToolchainError::Lock(error.to_string()))?;
    content.push('\n');
    fs::write(root.join(".bsmr-metadata.json"), content)
        .map_err(|error| write_error(root, error))?;
    fs::write(root.join(".gitignore"), "*\n!.gitignore\n")
        .map_err(|error| write_error(root, error))?;
    Ok(())
}

/// Replaces only a directory carrying Bessemer's acquisition marker.
fn replace_generated_directory(
    stage: tempfile::TempDir,
    destination: &Path,
) -> Result<(), GoToolchainError> {
    if destination.exists() {
        read_owned_acquisition(destination)?;
        fs::remove_dir_all(destination).map_err(|error| write_error(destination, error))?;
    }
    let stage = stage.keep();
    fs::rename(&stage, destination).map_err(|error| write_error(destination, error))?;
    Ok(())
}

/// Ensures an acquisition directory belongs to Bessemer before any mutation.
fn validate_owned_acquisition(path: &Path) -> Result<(), GoToolchainError> {
    read_owned_acquisition(path).map(|_| ())
}

/// Parses the structured marker required before mutating an acquisition directory.
fn read_owned_acquisition(path: &Path) -> Result<AcquiredSdk, GoToolchainError> {
    let marker = path.join(".bsmr-metadata.json");
    let bytes = fs::read(&marker).map_err(|_| GoToolchainError::UserOwned(path.to_owned()))?;
    let metadata = serde_json::from_slice::<AcquiredSdk>(&bytes)
        .map_err(|_| GoToolchainError::UserOwned(path.to_owned()))?;
    if metadata.generated_by != GENERATED_BY {
        return Err(GoToolchainError::UserOwned(path.to_owned()));
    }
    Ok(metadata)
}

/// Converts an I/O error into one path-specific generated-file failure.
fn write_error(path: &Path, error: std::io::Error) -> GoToolchainError {
    GoToolchainError::Write {
        path: path.to_owned(),
        message: error.to_string(),
    }
}
