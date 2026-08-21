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

//! Implements Bsmr's handling of target patterns and parsing of build files.

pub mod allow_relative_paths;
pub mod build_context;
pub mod coerce;
pub mod dice;
pub mod downstream_crate_starlark_defs;
pub mod extra;
pub mod factory;
pub mod file_loader;
pub mod file_type;
pub mod from_freeze;
pub mod import_paths;
pub mod late_binding_ty;
pub mod load_module;
pub mod package_imports;
pub mod parse_import;
pub mod paths;
pub mod plugins;
pub mod prelude_path;
pub mod print_handler;
pub mod soft_error;
pub mod starlark_debug;
pub mod starlark_profiler;
pub mod starlark_promise;
pub mod testing;
pub mod types;
