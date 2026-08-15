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

//! Interpreter related Dice calculations

use std::sync::Arc;

use allocative::Allocative;
use async_trait::async_trait;
use bsmr_common::package_listing::dice::DicePackageListingResolver;
use bsmr_core::bzl::ImportPath;
use bsmr_core::package::PackageLabel;
use bsmr_events::dispatch::async_record_root_spans;
use bsmr_events::span::SpanId;
use bsmr_interpreter::file_loader::LoadedModule;
use bsmr_interpreter::file_loader::ModuleDeps;
use bsmr_interpreter::load_module::INTERPRETER_CALCULATION_IMPL;
use bsmr_interpreter::load_module::InterpreterCalculationImpl;
use bsmr_interpreter::paths::module::OwnedStarlarkModulePath;
use bsmr_interpreter::paths::module::StarlarkModulePath;
use bsmr_interpreter::paths::package::PackageFilePath;
use bsmr_interpreter::paths::path::OwnedStarlarkPath;
use bsmr_interpreter::prelude_path::PreludePath;
use bsmr_node::nodes::eval_result::EvaluationResult;
use bsmr_node::nodes::frontend::TARGET_GRAPH_CALCULATION_IMPL;
use bsmr_node::nodes::frontend::TargetGraphCalculation;
use bsmr_node::nodes::frontend::TargetGraphCalculationImpl;
use bsmr_node::package_values_calculation::PACKAGE_VALUES_CALCULATION;
use bsmr_node::package_values_calculation::PackageValues;
use bsmr_node::package_values_calculation::PackageValuesCalculation;
use bsmr_util::time_span::TimeSpan;
use derive_more::Display;
use dice::DiceComputations;
use dice::Key;
use dice::OkPagableValueSerialize;
use dice::ValueSerialize;
use dice_futures::cancellation::CancellationContext;
use dupe::Dupe;
use futures::FutureExt;
use futures::future::BoxFuture;
use pagable::Pagable;
use pagable::pagable_typetag;
use smallvec::SmallVec;
use starlark::environment::Globals;

use crate::interpreter::dice_calculation_delegate::HasCalculationDelegate;
use crate::interpreter::dice_calculation_delegate::testing::EvalImportKey;
use crate::interpreter::global_interpreter_state::HasGlobalInterpreterState;
use crate::interpreter::package_file_calculation::EvalPackageFile;

// Key for 'InterpreterCalculation::get_interpreter_results'
#[derive(Clone, Dupe, Display, Debug, Eq, Hash, PartialEq, Allocative, Pagable)]
#[pagable_typetag(dice::DiceKeyDyn)]
pub struct InterpreterResultsKey(pub PackageLabel);

struct TargetGraphCalculationInstance;

pub(crate) fn init_target_graph_calculation_impl() {
    TARGET_GRAPH_CALCULATION_IMPL.init(&TargetGraphCalculationInstance);
}

#[async_trait]
impl Key for InterpreterResultsKey {
    type Value = bsmr_error::Result<Arc<EvaluationResult>>;
    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        cancellation: &CancellationContext,
    ) -> Self::Value {
        let ((time_span, result), spans) = async_record_root_spans(
            ctx.get_interpreter_results_uncached(self.0.dupe(), cancellation),
        )
        .await;

        ctx.store_evaluation_data(InterpreterResultsKeyActivationData {
            time_span,
            result: result.dupe(),
            spans,
        })?;

        result
    }

    fn equality(_: &Self::Value, _: &Self::Value) -> bool {
        // TODO consider if we want to impl eq for this
        false
    }

    fn validity(x: &Self::Value) -> bool {
        x.is_ok()
    }

    fn value_serialize() -> impl ValueSerialize<Value = Self::Value> {
        OkPagableValueSerialize::<Self::Value>::new()
    }
}

#[async_trait]
impl TargetGraphCalculationImpl for TargetGraphCalculationInstance {
    async fn get_interpreter_results_uncached(
        &self,
        ctx: &mut DiceComputations<'_>,
        package: PackageLabel,
        cancellation: &CancellationContext,
    ) -> (TimeSpan, bsmr_error::Result<Arc<EvaluationResult>>) {
        match ctx
            .get_interpreter_calculator(OwnedStarlarkPath::PackageFile(
                PackageFilePath::package_file_for_dir(package.as_cell_path()),
            ))
            .await
        {
            Ok(mut interpreter) => {
                interpreter
                    .eval_build_file(package.dupe(), cancellation)
                    .await
            }
            Err(e) => (TimeSpan::empty_now(), Err(e)),
        }
    }

    fn get_interpreter_results<'a>(
        &self,
        ctx: &'a mut DiceComputations,
        package: PackageLabel,
    ) -> BoxFuture<'a, bsmr_error::Result<Arc<EvaluationResult>>> {
        ctx.compute(&InterpreterResultsKey(package.dupe()))
            .map(|v| v?)
            .boxed()
    }
}

struct InterpreterCalculationInstance;
struct PackageValuesCalculationInstance;

pub(crate) fn init_interpreter_calculation_impl() {
    INTERPRETER_CALCULATION_IMPL.init(&InterpreterCalculationInstance);
    PACKAGE_VALUES_CALCULATION.init(&PackageValuesCalculationInstance);
}

#[async_trait]
impl Key for EvalImportKey {
    type Value = bsmr_error::Result<LoadedModule>;
    async fn compute(
        &self,
        ctx: &mut DiceComputations,
        cancellation: &CancellationContext,
    ) -> Self::Value {
        let starlark_path = self.0.borrow();
        // We cannot just use the inner default delegate's eval_import
        // because that wouldn't delegate back to us for inner eval_import calls.
        Ok(ctx
            .get_interpreter_calculator(OwnedStarlarkPath::new(starlark_path.starlark_path()))
            .await?
            .eval_module_uncached(starlark_path, cancellation)
            .await?)
    }

    fn equality(_: &Self::Value, _: &Self::Value) -> bool {
        // While it is technically possible to compare the modules
        // at least for simple modules (like modules defining only string constants),
        // practically it is too hard to make it work correctly for every case.
        false
    }

    fn validity(x: &Self::Value) -> bool {
        x.is_ok()
    }

    fn value_serialize() -> impl ValueSerialize<Value = Self::Value> {
        OkPagableValueSerialize::<Self::Value>::new()
    }
}

#[async_trait]
impl InterpreterCalculationImpl for InterpreterCalculationInstance {
    async fn get_loaded_module(
        &self,
        ctx: &mut DiceComputations<'_>,
        starlark_path: StarlarkModulePath<'_>,
    ) -> bsmr_error::Result<LoadedModule> {
        ctx.compute(&EvalImportKey(OwnedStarlarkModulePath::new(starlark_path)))
            .await?
    }

    async fn get_module_deps(
        &self,
        ctx: &mut DiceComputations<'_>,
        package: PackageLabel,
    ) -> bsmr_error::Result<ModuleDeps> {
        let mut listing = DicePackageListingResolver(ctx)
            .resolve_package_listing(package.dupe())
            .await?;

        let mut calc = ctx
            .get_interpreter_calculator(OwnedStarlarkPath::PackageFile(
                PackageFilePath::package_file_for_dir(package.as_cell_path()),
            ))
            .await?;

        let (_build_file_path, _module, module_deps) = calc
            .prepare_build_file_eval(package.dupe(), &mut listing)
            .await?;

        Ok(module_deps)
    }

    async fn get_package_file_deps(
        &self,
        ctx: &mut DiceComputations<'_>,
        package: PackageLabel,
    ) -> bsmr_error::Result<Option<(PackageFilePath, Vec<ImportPath>)>> {
        // These aren't cached on the DICE graph, since in normal evaluation there aren't that many, and we can cache at a higher level.
        // Therefore we re-parse the file, if it exists.
        // Fortunately, there are only a small number (currently a few hundred)
        let mut interpreter = ctx
            .get_interpreter_calculator(OwnedStarlarkPath::PackageFile(
                PackageFilePath::package_file_for_dir(package.as_cell_path()),
            ))
            .await?;
        let x = interpreter.prepare_package_file_eval(package).await?;
        let Some((package_file_path, _module, deps)) = x else {
            return Ok(None);
        };
        Ok(Some((
            package_file_path,
            deps.get_loaded_modules().imports().cloned().collect(),
        )))
    }

    async fn global_env(&self, ctx: &mut DiceComputations<'_>) -> bsmr_error::Result<Globals> {
        Ok(ctx.get_global_interpreter_state().await?.globals().dupe())
    }

    async fn prelude_import(
        &self,
        ctx: &mut DiceComputations<'_>,
    ) -> bsmr_error::Result<Option<PreludePath>> {
        Ok(ctx
            .get_global_interpreter_state()
            .await?
            .configuror
            .prelude_import()
            .cloned())
    }
}

#[async_trait]
impl PackageValuesCalculation for PackageValuesCalculationInstance {
    async fn package_values(
        &self,
        ctx: &mut DiceComputations<'_>,
        package: PackageLabel,
    ) -> bsmr_error::Result<PackageValues> {
        let super_package = ctx.eval_package_file(package).await?;
        Ok(PackageValues {
            package_values: super_package.package_values().package_values_json()?,
            visibility: super_package.visibility().to_json(),
            within_view: super_package.within_view().to_json(),
            visibility_cap: super_package.visibility_cap().to_json(),
        })
    }
}

pub struct InterpreterResultsKeyActivationData {
    /// TimeSpan of just the starlark evaluation of the build file.
    pub time_span: TimeSpan,
    pub result: bsmr_error::Result<Arc<EvaluationResult>>,
    pub spans: SmallVec<[SpanId; 1]>,
}
