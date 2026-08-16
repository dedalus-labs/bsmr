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

use bsmr_wrapper_common::DOT_BSMRCONFIG_D;

pub(crate) enum ExternalConfigSource {
    // Bsmrconfig file in the user's home directory
    UserFile(&'static str),

    // Bsmrconfig folder in the user's home directory, assuming all files in this folder are bsmrconfig
    UserFolder(&'static str),

    // Global bsmrconfig file. Repo related config is not allowed
    GlobalFile(&'static str),

    // Global bsmrconfig folder, assuming all files in this folder are bsmrconfig. Repo related config is not allowed
    GlobalFolder(&'static str),
}

pub(crate) enum ProjectConfigSource {
    // The cell's `.bsmr` configuration file.
    CellConfigFile,

    // Additional config file relative to the cell root, such as .bsmr.local
    CellRelativeFile(&'static str),

    // Bsmrconfig folder in the cell, assuming all files in this folder are bsmrconfig
    CellRelativeFolder(&'static str),
}

/// The default places from which bsmrconfigs are sourced.
///
/// Later entries take precedence over earlier ones, and project configs take precedence over
/// external configs.
pub(crate) static DEFAULT_EXTERNAL_CONFIG_SOURCES: &[ExternalConfigSource] = &[
    #[cfg(not(windows))]
    ExternalConfigSource::GlobalFolder("/etc/bsmrconfig.d"),
    #[cfg(not(windows))]
    ExternalConfigSource::GlobalFile("/etc/bsmrconfig"),
    // TODO: use %PROGRAMDATA% on Windows
    #[cfg(windows)]
    ExternalConfigSource::GlobalFolder("C:\\ProgramData\\bsmrconfig.d"),
    #[cfg(windows)]
    ExternalConfigSource::GlobalFile("C:\\ProgramData\\bsmrconfig"),
    ExternalConfigSource::UserFolder(DOT_BSMRCONFIG_D),
    ExternalConfigSource::UserFile(DOT_BSMRCONFIG_LOCAL),
];

pub(crate) static DEFAULT_PROJECT_CONFIG_SOURCES: &[ProjectConfigSource] = &[
    ProjectConfigSource::CellRelativeFolder(DOT_BSMRCONFIG_D),
    ProjectConfigSource::CellConfigFile,
    ProjectConfigSource::CellRelativeFile(DOT_BSMRCONFIG_LOCAL),
];

pub(crate) static DOT_BSMR: &str = ".bsmr";
pub(crate) static DOT_BSMRCONFIG_LOCAL: &str = ".bsmr.local";
