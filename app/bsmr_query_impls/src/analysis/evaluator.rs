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

//! Implementation of common cquery/uquery pieces.

use bsmr_common::scope::scope_and_collect_with_dispatcher;
use bsmr_error::BuckErrorContext;
use bsmr_events::dispatch::EventDispatcher;
use bsmr_query::query::environment::QueryEnvironment;
use bsmr_query::query::syntax::simple::eval::evaluator::QueryEvaluator;
use bsmr_query::query::syntax::simple::eval::literals::extract_target_literals;
use bsmr_query::query::syntax::simple::eval::multi_query::MultiQueryResult;
use bsmr_query::query::syntax::simple::eval::values::QueryEvaluationResult;
use bsmr_query::query::syntax::simple::eval::values::QueryEvaluationValue;
use bsmr_query::query::syntax::simple::functions::QueryFunctions;
use bsmr_query_parser::multi_query::MaybeMultiQuery;
use bsmr_query_parser::multi_query::MultiQueryItem;
use futures::Future;

pub(crate) async fn eval_query<
    F: QueryFunctions<Env = Env>,
    Env: QueryEnvironment,
    Fut: Future<Output = bsmr_error::Result<Env>> + Send,
>(
    dispatcher: EventDispatcher,
    functions: &F,
    query: &str,
    query_args: &[String],
    environment: impl Fn(Vec<String>) -> Fut + Send + Sync,
) -> bsmr_error::Result<QueryEvaluationResult<Env::Target>> {
    let query = MaybeMultiQuery::parse(query, query_args)?;
    match query {
        MaybeMultiQuery::MultiQuery(queries) => {
            let results = process_multi_query(dispatcher, functions, environment, &queries).await?;
            Ok(QueryEvaluationResult::Multiple(results))
        }
        MaybeMultiQuery::SingleQuery(query) => {
            let result = eval_single_query(functions, &query, environment).await?;
            Ok(QueryEvaluationResult::Single(result))
        }
    }
}

async fn eval_single_query<F, Env, Fut>(
    functions: &F,
    query: &str,
    environment: impl Fn(Vec<String>) -> Fut,
) -> bsmr_error::Result<QueryEvaluationValue<<Env as QueryEnvironment>::Target>>
where
    F: QueryFunctions<Env = Env>,
    Env: QueryEnvironment,
    Fut: Future<Output = bsmr_error::Result<Env>>,
{
    let literals = extract_target_literals(functions, query)?;
    let env = environment(literals).await?;
    QueryEvaluator::new(&env, functions).eval_query(query).await
}

async fn process_multi_query<Env, EnvFut, Qf>(
    dispatcher: EventDispatcher,
    functions: &Qf,
    env: impl Fn(Vec<String>) -> EnvFut + Send + Sync,
    queries: &[MultiQueryItem],
) -> bsmr_error::Result<MultiQueryResult<Env::Target>>
where
    Qf: QueryFunctions<Env = Env>,
    Env: QueryEnvironment,
    EnvFut: Future<Output = bsmr_error::Result<Env>> + Send,
{
    // SAFETY: it is safe as long as we don't forget the future. We don't do that.
    let ((), future_results) = unsafe {
        scope_and_collect_with_dispatcher(dispatcher, |scope| {
            for (i, query) in queries.iter().enumerate() {
                let arg: String = query.arg.clone();
                let arg_1: String = query.arg.clone();
                let env = &env;
                scope.spawn_cancellable(
                    async move {
                        let result = eval_single_query(functions, &query.query, env);
                        (i, arg, result.await)
                    },
                    move || {
                        (
                            i,
                            arg_1,
                            Err::<_, bsmr_error::Error>(bsmr_error::bsmr_error!(
                                bsmr_error::ErrorTag::Tier0,
                                "future was cancelled"
                            )),
                        )
                    },
                )
            }
        })
        .await
    };

    let mut results = Vec::with_capacity(future_results.len());
    for query_result in future_results {
        let (i, query, result) = query_result.buck_error_context("scope_and_collect failed")?;
        results.push((i, query, result));
    }
    results.sort_by_key(|(i, _, _)| *i);

    let map = results
        .into_iter()
        .map(|(_, query, result)| (query, result))
        .collect();
    Ok(MultiQueryResult(map))
}
