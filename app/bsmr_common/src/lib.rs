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

//! Common core components of bsmr

#![feature(map_try_insert)]
#![feature(used_with_arg)]

pub mod argv;
pub mod buckd_connection;
pub mod build_count;
pub mod buildfiles;
pub mod cargo_workspace;
pub mod cas_digest;
pub mod client_utils;
pub mod convert;
pub mod daemon_dir;
pub mod dice;
pub mod events;
pub mod external_cells;
pub mod external_symlink;
pub mod fbinit;
pub mod file_ops;
pub mod find_buildfile;
pub mod home_buck_tmp;
pub mod http;
pub mod ignores;
pub mod init;
pub mod invocation_paths;
pub mod invocation_paths_result;
pub mod invocation_roots;
pub mod io;
pub mod kill_util;
pub mod legacy_configs;
pub mod liveliness_observer;
pub mod local_resource_state;
pub mod memory;
pub mod package_boundary;
pub mod package_listing;
pub mod pattern;
pub mod pnpm_workspace;
pub mod python_lock;
pub mod python_project;
pub mod rlimits;
pub mod scope;
pub mod self_test_timeout;
pub mod sqlite;
pub mod starlark_profiler;
pub mod target_aliases;
pub mod temp_path;
pub mod tenting;
pub mod version_set;
