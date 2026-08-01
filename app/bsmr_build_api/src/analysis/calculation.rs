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

//! Rule analysis related Dice calculations
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;

use async_trait::async_trait;
use bsmr_core::configuration::compatibility::MaybeCompatible;
use bsmr_core::configuration::pair::ConfigurationNoExec;
use bsmr_core::provider::label::ConfiguredProvidersLabel;
use bsmr_core::provider::label::ProvidersLabel;
use bsmr_core::target::configured_target_label::ConfiguredTargetLabel;
use bsmr_node::nodes::configured::ConfiguredTargetNode;
use bsmr_node::nodes::configured_ref::ConfiguredGraphNodeRef;
use bsmr_query::query::syntax::simple::eval::set::TargetSet;
use bsmr_util::late_binding::LateBinding;
use dice::DiceComputations;
use dupe::Dupe;

use crate::analysis::AnalysisResult;
use crate::interpreter::rule_defs::provider::collection::FrozenProviderCollectionValue;
use crate::validation::transitive_validations::TransitiveValidations;

pub static EVAL_ANALYSIS_QUERY: LateBinding<
    for<'a> fn(
        &'a mut DiceComputations,
        &'a str,
        HashMap<String, ConfiguredTargetNode>,
    ) -> Pin<
        Box<dyn Future<Output = bsmr_error::Result<TargetSet<ConfiguredGraphNodeRef>>> + Send + 'a>,
    >,
> = LateBinding::new("EVAL_ANALYSIS_QUERY");

#[async_trait]
pub trait RuleAnalysisCalculationImpl: Send + Sync + 'static {
    async fn get_analysis_result(
        &self,
        ctx: &mut DiceComputations<'_>,
        target: &ConfiguredTargetLabel,
    ) -> bsmr_error::Result<MaybeCompatible<AnalysisResult>>;
}

pub static RULE_ANALYSIS_CALCULATION: LateBinding<&'static dyn RuleAnalysisCalculationImpl> =
    LateBinding::new("RULE_ANALYSIS_CALCULATION");

#[async_trait]
pub trait RuleAnalysisCalculation {
    /// Returns the analysis result for a ConfiguredTargetLabel. This is the full set of Providers
    /// returned by the target's rule implementation function.
    async fn get_analysis_result(
        &mut self,
        target: &ConfiguredTargetLabel,
    ) -> bsmr_error::Result<MaybeCompatible<AnalysisResult>>;

    /// Return the analysis result for a configuration rule `TargetLabel`
    /// (e. g. `constraint_value`).
    async fn get_configuration_analysis_result(
        &mut self,
        target: &ProvidersLabel,
    ) -> bsmr_error::Result<FrozenProviderCollectionValue>;

    /// Returns the provider collection for a ConfiguredProvidersLabel. This is the full set of Providers
    /// returned by the target's rule implementation function.
    async fn get_providers(
        &mut self,
        target: &ConfiguredProvidersLabel,
    ) -> bsmr_error::Result<MaybeCompatible<FrozenProviderCollectionValue>>;

    async fn get_validations(
        &mut self,
        target: &ConfiguredTargetLabel,
    ) -> bsmr_error::Result<MaybeCompatible<Option<TransitiveValidations>>>;
}

#[async_trait]
impl RuleAnalysisCalculation for DiceComputations<'_> {
    async fn get_analysis_result(
        &mut self,
        target: &ConfiguredTargetLabel,
    ) -> bsmr_error::Result<MaybeCompatible<AnalysisResult>> {
        RULE_ANALYSIS_CALCULATION
            .get()?
            .get_analysis_result(self, target)
            .await
    }

    async fn get_configuration_analysis_result(
        &mut self,
        target: &ProvidersLabel,
    ) -> bsmr_error::Result<FrozenProviderCollectionValue> {
        // Analysis for configuration nodes is always done with the unbound configuration.
        let target = target.configure_pair(ConfigurationNoExec::unbound().cfg_pair().dupe());
        Ok(self.get_providers(&target).await?.require_compatible()?)
    }

    async fn get_providers(
        &mut self,
        target: &ConfiguredProvidersLabel,
    ) -> bsmr_error::Result<MaybeCompatible<FrozenProviderCollectionValue>> {
        let analysis = self.get_analysis_result(target.target()).await?;

        analysis.try_map(|analysis| analysis.lookup_inner(target))
    }

    async fn get_validations(
        &mut self,
        target: &ConfiguredTargetLabel,
    ) -> bsmr_error::Result<MaybeCompatible<Option<TransitiveValidations>>> {
        let analysis = self.get_analysis_result(target).await?;
        Ok(analysis.map(|x| x.validations))
    }
}
