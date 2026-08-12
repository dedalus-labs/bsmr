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

use bsmr_core::plugins::PluginKind;
use bsmr_util::late_binding::LateBinding;
use starlark::values::Value;

pub static PLUGIN_KIND_FROM_VALUE: LateBinding<
    for<'v> fn(Value<'v>) -> bsmr_error::Result<PluginKind>,
> = LateBinding::new("PLUGIN_KIND_FROM_VALUE");
