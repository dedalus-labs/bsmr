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

#![feature(used_with_arg)]
#![feature(anonymous_lifetime_in_impl_trait)]

pub mod bxl;
pub mod commands;
pub mod concurrency;
pub mod ctx;
pub mod global_cfg_options;
pub mod late_bindings;
pub mod partial_result_dispatcher;
pub mod pattern_parse_and_resolve;
pub mod stderr_output_guard;
pub mod stdout_partial_output;
pub mod streaming_request_handler;
pub mod target_resolution_config;
pub mod template;
pub mod test_command;
