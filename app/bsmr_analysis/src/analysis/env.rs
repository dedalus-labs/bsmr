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

use std::sync::Arc;
use std::time::Instant;

use bsmr_build_api::analysis::AnalysisResult;
use bsmr_build_api::analysis::anon_promises_dyn::RunAnonPromisesAccessorPair;
use bsmr_build_api::analysis::registry::AnalysisRegistry;
use bsmr_build_api::interpreter::rule_defs::cmd_args::value::FrozenCommandLineArg;
use bsmr_build_api::interpreter::rule_defs::context::AnalysisContext;
use bsmr_build_api::interpreter::rule_defs::provider::builtin::template_placeholder_info::FrozenTemplatePlaceholderInfo;
use bsmr_build_api::interpreter::rule_defs::provider::builtin::validation_info::FrozenValidationInfo;
use bsmr_build_api::interpreter::rule_defs::provider::collection::FrozenProviderCollection;
use bsmr_build_api::interpreter::rule_defs::provider::collection::FrozenProviderCollectionValue;
use bsmr_build_api::interpreter::rule_defs::provider::collection::FrozenProviderCollectionValueRef;
use bsmr_build_api::interpreter::rule_defs::provider::collection::ProviderCollection;
use bsmr_build_api::validation::transitive_validations::TransitiveValidations;
use bsmr_build_api::validation::transitive_validations::TransitiveValidationsData;
use bsmr_core::deferred::base_deferred_key::BaseDeferredKey;
use bsmr_core::execution_types::execution::ExecutionPlatformResolution;
use bsmr_core::provider::label::ConfiguredProvidersLabel;
use bsmr_core::target::configured_target_label::ConfiguredTargetLabel;
use bsmr_core::unsafe_send_future::UnsafeSendFuture;
use bsmr_error::BuckErrorContext;
use bsmr_error::conversion::from_any_with_tag;
use bsmr_error::internal_error;
use bsmr_events::dispatch::get_dispatcher;
use bsmr_execute::digest_config::HasDigestConfig;
use bsmr_hash::StdBuckHashMap;
use bsmr_interpreter::dice::starlark_provider::StarlarkEvalKind;
use bsmr_interpreter::factory::BuckStarlarkModule;
use bsmr_interpreter::factory::StarlarkEvaluatorProvider;
use bsmr_interpreter::print_handler::EventDispatcherPrintHandler;
use bsmr_interpreter::soft_error::BsmrStarlarkSoftErrorHandler;
use bsmr_interpreter::types::rule::FROZEN_PROMISE_ARTIFACT_MAPPINGS_GET_IMPL;
use bsmr_interpreter::types::rule::FROZEN_RULE_GET_IMPL;
use bsmr_node::nodes::configured::ConfiguredTargetNodeRef;
use bsmr_node::rule_type::StarlarkRuleType;
use dice::CancellationContext;
use dice::DiceComputations;
use dupe::Dupe;
use futures::Future;
use starlark::environment::FrozenModule;
use starlark::environment::Module;
use starlark::eval::Evaluator;
use starlark::values::FrozenValue;
use starlark::values::FrozenValueTyped;
use starlark::values::Value;
use starlark::values::ValueTyped;
use starlark::values::ValueTypedComplex;
use starlark_map::small_map::SmallMap;

use crate::analysis::calculation::AnalysisSplitInstants;
use crate::analysis::plugins::plugins_to_starlark_value;
use crate::attrs::resolve::ctx::AnalysisQueryResult;
use crate::attrs::resolve::ctx::AttrResolutionContext;
use crate::attrs::resolve::node_to_attrs_struct::node_to_attrs_struct;

#[derive(bsmr_error::Error, Debug)]
#[bsmr(tag = Tier0)]
enum AnalysisError {
    #[error(
        "Analysis context was missing a query result, this shouldn't be possible. Query was `{0}`"
    )]
    MissingQuery(String),
    #[error("required dependency `{0}` was not found")]
    MissingDep(ConfiguredProvidersLabel),
}

// Contains a `module` that things must live on, and various `FrozenProviderCollectionValue`s
// that are NOT tied to that module. Must claim ownership of them via `add_reference` before returning them.
pub struct RuleAnalysisAttrResolutionContext<'a, 'v> {
    pub module: &'a Module<'v>,
    pub dep_analysis_results: StdBuckHashMap<ConfiguredTargetLabel, FrozenProviderCollectionValue>,
    pub query_results: StdBuckHashMap<String, Arc<AnalysisQueryResult>>,
    pub execution_platform_resolution: ExecutionPlatformResolution,
}

impl<'a, 'v> AttrResolutionContext<'v> for &'_ RuleAnalysisAttrResolutionContext<'a, 'v> {
    fn starlark_module(&self) -> &Module<'v> {
        self.module
    }

    fn get_dep(
        &mut self,
        target: &ConfiguredProvidersLabel,
    ) -> bsmr_error::Result<FrozenValueTyped<'v, FrozenProviderCollection>> {
        get_dep(&self.dep_analysis_results, target, self.module)
    }

    fn resolve_unkeyed_placeholder(
        &mut self,
        name: &str,
    ) -> bsmr_error::Result<Option<FrozenCommandLineArg>> {
        Ok(resolve_unkeyed_placeholder(
            &self.dep_analysis_results,
            name,
            self.module,
        ))
    }

    fn resolve_query(&mut self, query: &str) -> bsmr_error::Result<Arc<AnalysisQueryResult>> {
        resolve_query(&self.query_results, query, self.module)
    }

    fn execution_platform_resolution(&self) -> &ExecutionPlatformResolution {
        &self.execution_platform_resolution
    }
}

pub fn get_dep<'v>(
    dep_analysis_results: &StdBuckHashMap<ConfiguredTargetLabel, FrozenProviderCollectionValue>,
    target: &ConfiguredProvidersLabel,
    module: &Module<'v>,
) -> bsmr_error::Result<FrozenValueTyped<'v, FrozenProviderCollection>> {
    match dep_analysis_results.get(target.target()) {
        None => Err(AnalysisError::MissingDep(target.dupe()).into()),
        Some(x) => {
            let x = x.lookup_inner(target)?;
            // IMPORTANT: Anything given back to the user must be kept alive
            Ok(x.add_heap_ref(module.heap()))
        }
    }
}

pub fn resolve_unkeyed_placeholder(
    dep_analysis_results: &StdBuckHashMap<ConfiguredTargetLabel, FrozenProviderCollectionValue>,
    name: &str,
    module: &Module,
) -> Option<FrozenCommandLineArg> {
    // TODO(cjhopman): Make it an error if two deps provide a value for the placeholder.
    for providers in dep_analysis_results.values() {
        if let Some(placeholder_info) = providers
            .provider_collection()
            .builtin_provider::<FrozenTemplatePlaceholderInfo>()
        {
            if let Some(value) = placeholder_info.unkeyed_variables().get(name) {
                // IMPORTANT: Anything given back to the user must be kept alive
                module
                    .frozen_heap()
                    .add_reference(providers.value().owner());
                return Some(*value);
            }
        }
    }
    None
}

pub fn resolve_query(
    query_results: &StdBuckHashMap<String, Arc<AnalysisQueryResult>>,
    query: &str,
    module: &Module,
) -> bsmr_error::Result<Arc<AnalysisQueryResult>> {
    match query_results.get(query) {
        None => Err(AnalysisError::MissingQuery(query.to_owned()).into()),
        Some(x) => {
            for (_, y) in x.result.iter() {
                // IMPORTANT: Anything given back to the user must be kept alive
                module.frozen_heap().add_reference(y.value().owner());
            }
            Ok(x.dupe())
        }
    }
}

pub trait RuleSpec: Sync {
    fn invoke<'v>(
        &self,
        eval: &mut Evaluator<'v, '_, '_>,
        ctx: ValueTyped<'v, AnalysisContext<'v>>,
    ) -> bsmr_error::Result<Value<'v>>;

    fn promise_artifact_mappings<'v>(
        &self,
        eval: &mut Evaluator<'v, '_, '_>,
    ) -> bsmr_error::Result<SmallMap<String, Value<'v>>>;
}

/// Container for the environment that analysis implementation functions should run in
struct AnalysisEnv<'a> {
    rule_spec: &'a dyn RuleSpec,
    deps: Vec<(&'a ConfiguredTargetLabel, AnalysisResult)>,
    query_results: StdBuckHashMap<String, Arc<AnalysisQueryResult>>,
    execution_platform: &'a ExecutionPlatformResolution,
    label: ConfiguredTargetLabel,
    cancellation: &'a CancellationContext,
}

pub(crate) async fn run_analysis<'a>(
    dice: &'a mut DiceComputations<'_>,
    label: &ConfiguredTargetLabel,
    results: Vec<(&'a ConfiguredTargetLabel, AnalysisResult)>,
    query_results: StdBuckHashMap<String, Arc<AnalysisQueryResult>>,
    execution_platform: &'a ExecutionPlatformResolution,
    rule_spec: &'a dyn RuleSpec,
    node: ConfiguredTargetNodeRef<'a>,
    cancellation: &CancellationContext,
) -> bsmr_error::Result<(AnalysisResult, Option<AnalysisSplitInstants>)> {
    let analysis_env = AnalysisEnv {
        rule_spec,
        deps: results,
        query_results,
        execution_platform,
        label: label.dupe(),
        cancellation,
    };
    run_analysis_with_env(dice, analysis_env, node).await
}

pub fn get_deps_from_analysis_results(
    results: Vec<(&ConfiguredTargetLabel, AnalysisResult)>,
) -> bsmr_error::Result<StdBuckHashMap<ConfiguredTargetLabel, FrozenProviderCollectionValue>> {
    results
        .into_iter()
        .map(|(label, result)| Ok((label.dupe(), result.providers()?.to_owned())))
        .collect::<bsmr_error::Result<StdBuckHashMap<ConfiguredTargetLabel, FrozenProviderCollectionValue>>>()
}

// Used to express that the impl Future below captures multiple named lifetimes.
// See https://github.com/rust-lang/rust/issues/34511#issuecomment-373423999 for more details.
trait Captures<'x> {}
impl<T: ?Sized> Captures<'_> for T {}

fn run_analysis_with_env<'a, 'd: 'a>(
    dice: &'a mut DiceComputations<'d>,
    analysis_env: AnalysisEnv<'a>,
    node: ConfiguredTargetNodeRef<'a>,
) -> impl Future<Output = bsmr_error::Result<(AnalysisResult, Option<AnalysisSplitInstants>)>>
+ 'a
+ Captures<'d> {
    let fut = async move { run_analysis_with_env_underlying(dice, analysis_env, node).await };
    unsafe { UnsafeSendFuture::new_encapsulates_starlark(fut) }
}

async fn run_analysis_with_env_underlying(
    dice: &mut DiceComputations<'_>,
    analysis_env: AnalysisEnv<'_>,
    node: ConfiguredTargetNodeRef<'_>,
) -> bsmr_error::Result<(AnalysisResult, Option<AnalysisSplitInstants>)> {
    BuckStarlarkModule::with_profiling_async(async move |env| {
        let print = EventDispatcherPrintHandler(get_dispatcher());

        let validations_from_deps = analysis_env
            .deps
            .iter()
            .filter_map(|(label, analysis_result)| {
                analysis_result
                    .validations
                    .dupe()
                    .map(|v| ((*label).dupe(), v))
            })
            .collect::<SmallMap<_, _>>();

        let (attributes, plugins) = {
            let dep_analysis_results = get_deps_from_analysis_results(analysis_env.deps)?;
            let resolution_ctx = RuleAnalysisAttrResolutionContext {
                module: &env,
                dep_analysis_results,
                query_results: analysis_env.query_results,
                execution_platform_resolution: node.execution_platform_resolution().clone(),
            };

            (
                node_to_attrs_struct(node, &mut &resolution_ctx)?,
                plugins_to_starlark_value(node, &mut &resolution_ctx)?,
            )
        };

        let registry = AnalysisRegistry::new_from_owner(
            BaseDeferredKey::TargetLabel(node.label().dupe()),
            analysis_env.execution_platform.dupe(),
        )?;

        let eval_kind = StarlarkEvalKind::Analysis(node.label().dupe());
        let eval_provider = StarlarkEvaluatorProvider::new(dice, eval_kind).await?;
        let mut reentrant_eval =
            eval_provider.make_reentrant_evaluator(&env, analysis_env.cancellation.into())?;

        let (ctx, list_res) = reentrant_eval.with_evaluator(|eval| {
            eval.set_print_handler(&print);
            eval.set_soft_error_handler(&BsmrStarlarkSoftErrorHandler);

            let ctx = AnalysisContext::prepare(
                eval.heap(),
                Some(attributes),
                Some(analysis_env.label),
                Some(plugins.into()),
                registry,
                dice.global_data().get_digest_config(),
            );

            let list_res = analysis_env.rule_spec.invoke(eval, ctx)?;

            Ok((ctx, list_res))
        })?;

        let pre_promises = Instant::now();
        let resolved_any = ctx
            .actions
            .run_promises(&mut RunAnonPromisesAccessorPair(&mut reentrant_eval, dice))
            .await?;
        let post_promises = Instant::now();

        let split_instants = if resolved_any {
            Some(AnalysisSplitInstants {
                pre_promises,
                post_promises,
            })
        } else {
            None
        };

        // Pull the ctx object back out, and steal ctx.action's state back
        let analysis_registry = ctx.take_state();

        // TODO: Convert the ValueError from `try_from_value` better than just printing its Debug
        let res_typed = ProviderCollection::try_from_value(list_res)?;
        {
            let provider_collection = ValueTypedComplex::new_err(env.heap().alloc(res_typed))
                .internal_error("Just allocated provider collection")?;
            analysis_registry
                .analysis_value_storage
                .set_result_value(provider_collection)?;
        }

        let finished_eval = reentrant_eval.finish_evaluation();

        let declared_actions = analysis_registry.num_declared_actions();
        let declared_artifacts = analysis_registry.num_declared_artifacts();
        let registry_finalizer = analysis_registry.finalize(&env)?;
        let (token, frozen_env, profile_data) = finished_eval.freeze_and_finish(env)?;
        let recorded_values = registry_finalizer(&frozen_env)?;

        let validations = transitive_validations(
            validations_from_deps,
            recorded_values.provider_collection()?,
        );

        Ok((
            token,
            (
                AnalysisResult::new(
                    recorded_values,
                    profile_data,
                    StdBuckHashMap::default(),
                    declared_actions,
                    declared_artifacts,
                    validations,
                ),
                split_instants,
            ),
        ))
    })
    .await
}

pub fn transitive_validations(
    deps: SmallMap<ConfiguredTargetLabel, TransitiveValidations>,
    provider_collection: FrozenProviderCollectionValueRef,
) -> Option<TransitiveValidations> {
    let provider_collection = provider_collection.to_owned();
    let info = provider_collection
        .value
        .maybe_map(|c| c.as_ref().builtin_provider_value::<FrozenValidationInfo>());
    if info.is_some() || deps.len() > 1 {
        Some(TransitiveValidations(Arc::new(TransitiveValidationsData {
            info,
            children: deps.into_keys().collect(),
        })))
    } else {
        assert!(
            deps.len() <= 1,
            "Reuse the single element if any from one of the deps for current node."
        );
        deps.into_values().next()
    }
}

fn get_rule_callable(
    eval: &mut Evaluator<'_, '_, '_>,
    module: &FrozenModule,
    name: &str,
) -> bsmr_error::Result<FrozenValue> {
    let rule_callable = module
        .get_any_visibility(name)
        .map_err(|e| from_any_with_tag(e, bsmr_error::ErrorTag::Tier0))
        .with_buck_error_context(|| format!("Couldn't find rule `{name}`"))?
        .0;
    let rule_callable = rule_callable.owned_value(eval.frozen_heap());
    let rule_callable = rule_callable
        .unpack_frozen()
        .ok_or_else(|| internal_error!("Must be frozen"))?;
    Ok(rule_callable)
}

pub fn get_rule_impl(
    eval: &mut Evaluator<'_, '_, '_>,
    module: &FrozenModule,
    name: &str,
) -> bsmr_error::Result<FrozenValue> {
    let rule_callable = get_rule_callable(eval, module, name)?;
    let rule_impl = (FROZEN_RULE_GET_IMPL.get()?)(rule_callable)?;
    Ok(rule_impl)
}

pub fn promise_artifact_mappings<'v>(
    eval: &mut Evaluator<'v, '_, '_>,
    module: &FrozenModule,
    name: &str,
) -> bsmr_error::Result<SmallMap<String, Value<'v>>> {
    let rule_callable = get_rule_callable(eval, module, name)?;
    let frozen_promise_artifact_mappings =
        (FROZEN_PROMISE_ARTIFACT_MAPPINGS_GET_IMPL.get()?)(rule_callable)?;

    Ok(frozen_promise_artifact_mappings
        .iter()
        .map(|(frozen_string, frozen_func)| (frozen_string.to_string(), frozen_func.to_value()))
        .collect::<SmallMap<_, _>>())
}

pub fn get_user_defined_rule_spec(
    module: FrozenModule,
    rule_type: &StarlarkRuleType,
) -> impl RuleSpec + use<> {
    struct Impl {
        module: FrozenModule,
        name: String,
    }

    impl RuleSpec for Impl {
        fn invoke<'v>(
            &self,
            eval: &mut Evaluator<'v, '_, '_>,
            ctx: ValueTyped<'v, AnalysisContext<'v>>,
        ) -> bsmr_error::Result<Value<'v>> {
            let rule_impl = get_rule_impl(eval, &self.module, &self.name)?;
            Ok(eval.eval_function(rule_impl.to_value(), &[ctx.to_value()], &[])?)
        }

        fn promise_artifact_mappings<'v>(
            &self,
            eval: &mut Evaluator<'v, '_, '_>,
        ) -> bsmr_error::Result<SmallMap<String, Value<'v>>> {
            promise_artifact_mappings(eval, &self.module, &self.name)
        }
    }

    Impl {
        module,
        name: rule_type.name.clone(),
    }
}
