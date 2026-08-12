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

use std::pin::Pin;

use bsmr_artifact::artifact::artifact_type::Artifact;
use bsmr_build_api::analysis::registry::RecordedAnalysisValues;
use bsmr_build_api::dynamic_value::DynamicValue;
use bsmr_build_api::interpreter::rule_defs::provider::collection::FrozenProviderCollectionValue;
use bsmr_core::deferred::base_deferred_key::BaseDeferredKeyBxl;
use bsmr_core::deferred::dynamic::DynamicLambdaResultsKey;
use bsmr_execute::artifact_value::ArtifactValue;
use bsmr_execute::digest_config::DigestConfig;
use bsmr_hash::BuckIndexMap;
use bsmr_hash::StdBuckHashMap;
use bsmr_util::late_binding::LateBinding;
use dice::DiceComputations;
use dice_futures::cancellation::CancellationObserver;
use futures::Future;
use starlark::values::OwnedRefFrozenRef;

use crate::dynamic::deferred::InputArtifactsMaterialized;
use crate::dynamic::params::FrozenDynamicLambdaParams;

pub static EVAL_BXL_FOR_DYNAMIC_OUTPUT: LateBinding<
    for<'v> fn(
        &'v BaseDeferredKeyBxl,
        DynamicLambdaResultsKey,
        OwnedRefFrozenRef<'v, FrozenDynamicLambdaParams>,
        &'v mut DiceComputations,
        InputArtifactsMaterialized,
        &'v BuckIndexMap<&Artifact, &ArtifactValue>,
        StdBuckHashMap<DynamicValue, FrozenProviderCollectionValue>,
        DigestConfig,
        CancellationObserver,
    ) -> Pin<
        Box<dyn Future<Output = bsmr_error::Result<RecordedAnalysisValues>> + Send + 'v>,
    >,
> = LateBinding::new("EVAL_BXL_FOR_DYNAMIC_OUTPUT");

pub(crate) async fn eval_bxl_for_dynamic_output<'v>(
    base_deferred_key: &'v BaseDeferredKeyBxl,
    self_key: DynamicLambdaResultsKey,
    dynamic_lambda: OwnedRefFrozenRef<'_, FrozenDynamicLambdaParams>,
    dice_ctx: &'v mut DiceComputations<'_>,
    input_artifacts_materialized: InputArtifactsMaterialized,
    ensured_artifacts: &'v BuckIndexMap<&Artifact, &ArtifactValue>,
    resolved_dynamic_values: StdBuckHashMap<DynamicValue, FrozenProviderCollectionValue>,
    digest_config: DigestConfig,
    liveness: CancellationObserver,
) -> bsmr_error::Result<RecordedAnalysisValues> {
    (EVAL_BXL_FOR_DYNAMIC_OUTPUT.get()?)(
        base_deferred_key,
        self_key,
        dynamic_lambda,
        dice_ctx,
        input_artifacts_materialized,
        ensured_artifacts,
        resolved_dynamic_values,
        digest_config,
        liveness,
    )
    .await
}
