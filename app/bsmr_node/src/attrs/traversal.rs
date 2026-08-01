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

use std::sync::Arc;

use bsmr_core::configuration::transition::id::TransitionId;
use bsmr_core::package::source_path::SourcePathRef;
use bsmr_core::plugins::PluginKind;
use bsmr_core::provider::label::ProvidersLabel;
use bsmr_core::target::label::label::TargetLabel;
use dupe::Dupe;

use crate::attrs::attr_type::configuration_dep::ConfigurationDepKind;

pub trait CoercedAttrTraversal<'a> {
    fn dep(&mut self, dep: &ProvidersLabel) -> bsmr_error::Result<()>;
    fn exec_dep(&mut self, dep: &'a ProvidersLabel) -> bsmr_error::Result<()> {
        self.dep(dep)
    }

    fn toolchain_dep(&mut self, dep: &'a ProvidersLabel) -> bsmr_error::Result<()> {
        self.dep(dep)
    }

    fn transition_dep(
        &mut self,
        dep: &'a ProvidersLabel,
        _tr: &Arc<TransitionId>,
    ) -> bsmr_error::Result<()> {
        self.dep(dep)
    }

    fn split_transition_dep(
        &mut self,
        dep: &'a ProvidersLabel,
        _tr: &Arc<TransitionId>,
    ) -> bsmr_error::Result<()> {
        self.dep(dep)
    }

    fn configuration_dep(
        &mut self,
        dep: &ProvidersLabel,
        _kind: ConfigurationDepKind,
    ) -> bsmr_error::Result<()> {
        self.dep(dep)
    }

    fn plugin_dep(&mut self, dep: &'a TargetLabel, _kind: &PluginKind) -> bsmr_error::Result<()> {
        let p = ProvidersLabel::default_for(dep.dupe());
        self.dep(&p)
    }

    fn input(&mut self, input: SourcePathRef) -> bsmr_error::Result<()>;

    fn label(&mut self, _label: &'a ProvidersLabel) -> bsmr_error::Result<()> {
        Ok(())
    }
}
