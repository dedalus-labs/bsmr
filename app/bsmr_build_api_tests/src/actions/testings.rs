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

use std::borrow::Cow;

use allocative::Allocative;
use async_trait::async_trait;
use bsmr_artifact::artifact::build_artifact::BuildArtifact;
use bsmr_build_api::actions::Action;
use bsmr_build_api::actions::ActionExecutionCtx;
use bsmr_build_api::actions::UnregisteredAction;
use bsmr_build_api::actions::box_slice_set::BoxSliceSet;
use bsmr_build_api::actions::execute::action_executor::ActionExecutionMetadata;
use bsmr_build_api::actions::execute::action_executor::ActionOutputs;
use bsmr_build_api::actions::execute::error::ExecuteError;
use bsmr_build_api::artifact_groups::ArtifactGroup;
use bsmr_build_signals::env::WaitingData;
use bsmr_core::category::Category;
use bsmr_core::category::CategoryRef;
use bsmr_execute::execute::request::CommandExecutionOutput;
use bsmr_execute::execute::request::CommandExecutionPaths;
use bsmr_execute::execute::request::CommandExecutionRequest;
use bsmr_execute::execute::request::OutputType;
use bsmr_hash::BsmrIndexSet;
use derivative::Derivative;
use dupe::Dupe;
use pagable::Pagable;
use pagable::pagable_typetag;
use sorted_vector_map::sorted_vector_map;
use starlark::values::OwnedFrozenValue;

/// A simple unregistered action that will eventually be resolved into an action that runs the
/// given cmd as the action execution command. Used for testing
///
/// This action is for testing, and bypasses the need to create starlark values and frozen
/// modules
#[derive(Allocative, Clone, PartialEq)]
pub(crate) struct SimpleUnregisteredAction {
    inputs: BsmrIndexSet<ArtifactGroup>,
    cmd: Vec<String>,
    category: Category,
    identifier: Option<String>,
}

impl SimpleUnregisteredAction {
    pub(crate) fn new(
        inputs: BsmrIndexSet<ArtifactGroup>,
        cmd: Vec<String>,
        category: Category,
        identifier: Option<String>,
    ) -> Self {
        Self {
            inputs,
            cmd,
            category,
            identifier,
        }
    }
}

/// The action created by SimpleUnregisteredAction, or directly.
#[derive(Derivative, Allocative, Pagable)]
#[derivative(Debug)]
pub(crate) struct SimpleAction {
    inputs: BoxSliceSet<ArtifactGroup>,
    outputs: BoxSliceSet<BuildArtifact>,
    cmd: Vec<String>,
    category: Category,
    identifier: Option<String>,
}

impl SimpleAction {
    pub(crate) fn new(
        inputs: BsmrIndexSet<ArtifactGroup>,
        outputs: BsmrIndexSet<BuildArtifact>,
        cmd: Vec<String>,
        category: Category,
        identifier: Option<String>,
    ) -> Self {
        Self {
            inputs: BoxSliceSet::from(inputs),
            outputs: BoxSliceSet::from(outputs),
            cmd,
            category,
            identifier,
        }
    }
}

impl UnregisteredAction for SimpleUnregisteredAction {
    fn register(
        self: Box<Self>,
        outputs: BsmrIndexSet<BuildArtifact>,
        _starlark_data: Option<OwnedFrozenValue>,
        _error_handler: Option<OwnedFrozenValue>,
    ) -> bsmr_error::Result<Box<dyn Action>> {
        Ok(Box::new(SimpleAction {
            inputs: BoxSliceSet::from(self.inputs),
            outputs: BoxSliceSet::from(outputs),
            cmd: self.cmd,
            category: self.category,
            identifier: self.identifier,
        }))
    }
}

#[pagable_typetag]
#[async_trait]
impl Action for SimpleAction {
    fn kind(&self) -> bsmr_data::ActionKind {
        bsmr_data::ActionKind::NotSet
    }

    fn inputs(&self) -> bsmr_error::Result<Cow<'_, [ArtifactGroup]>> {
        Ok(Cow::Borrowed(self.inputs.as_slice()))
    }

    fn outputs(&self) -> Cow<'_, [BuildArtifact]> {
        Cow::Borrowed(self.outputs.as_slice())
    }

    fn first_output(&self) -> &BuildArtifact {
        &self.outputs.as_slice()[0]
    }

    fn category(&self) -> CategoryRef<'_> {
        self.category.as_ref()
    }

    fn identifier(&self) -> Option<&str> {
        self.identifier.as_deref()
    }

    async fn execute(
        &self,
        ctx: &mut dyn ActionExecutionCtx,
        _waiting_data: WaitingData,
    ) -> Result<(ActionOutputs, ActionExecutionMetadata), ExecuteError> {
        let req = CommandExecutionRequest::new(
            vec![],
            self.cmd.clone(),
            CommandExecutionPaths::new(
                Vec::new(),
                self.outputs
                    .iter()
                    .map(|b| CommandExecutionOutput::BuildArtifact {
                        path: b.get_path().dupe(),
                        output_type: OutputType::File,
                    })
                    .collect(),
                ctx.fs(),
                ctx.digest_config(),
                None,
            )?,
            sorted_vector_map![],
        );

        let prepared_action = ctx.prepare_action(&req, true)?;
        let manager = ctx.command_execution_manager(WaitingData::new());
        let result = ctx.exec_cmd(manager, &req, &prepared_action).await;
        let (outputs, meta) = ctx.unpack_command_execution_result(
            req.executor_preference,
            result,
            false,
            false,
            None,
            bsmr_data::IncrementalKind::NonIncremental,
        )?;

        Ok((outputs, meta))
    }
}
