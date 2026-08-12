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

use bsmr_core::package::source_path::SourcePathRef;
use bsmr_core::plugins::PluginKind;
use bsmr_core::plugins::PluginKindSet;
use bsmr_core::provider::label::ConfiguredProvidersLabel;
use bsmr_core::provider::label::ProvidersLabel;
use bsmr_core::target::label::label::TargetLabel;

use crate::attrs::attr_type::query::ResolvedQueryLiterals;

pub trait ConfiguredAttrTraversal {
    fn dep(&mut self, dep: &ConfiguredProvidersLabel) -> bsmr_error::Result<()>;

    fn dep_with_plugins(
        &mut self,
        dep: &ConfiguredProvidersLabel,
        _plugins: &PluginKindSet,
    ) -> bsmr_error::Result<()> {
        // By default, just treat it as a dep. Most things don't care about the distinction.
        self.dep(dep)
    }

    fn exec_dep(&mut self, dep: &ConfiguredProvidersLabel) -> bsmr_error::Result<()> {
        // By default, just treat it as a dep. Most things don't care about the distinction.
        self.dep(dep)
    }

    fn toolchain_dep(&mut self, dep: &ConfiguredProvidersLabel) -> bsmr_error::Result<()> {
        // By default, just treat it as a dep. Most things don't care about the distinction.
        self.dep(dep)
    }

    fn configuration_dep(&mut self, _dep: &ProvidersLabel) -> bsmr_error::Result<()> {
        Ok(())
    }

    fn plugin_dep(&mut self, _dep: &TargetLabel, _kind: &PluginKind) -> bsmr_error::Result<()> {
        Ok(())
    }

    /// Called for both `attrs.query(...)` and query macros like `$(query_targets ...)`.
    fn query(
        &mut self,
        _query: &str,
        _resolved_literals: &ResolvedQueryLiterals<ConfiguredProvidersLabel>,
    ) -> bsmr_error::Result<()> {
        Ok(())
    }

    fn input(&mut self, _path: SourcePathRef) -> bsmr_error::Result<()> {
        Ok(())
    }

    fn label(&mut self, _label: &ConfiguredProvidersLabel) -> bsmr_error::Result<()> {
        Ok(())
    }
}
