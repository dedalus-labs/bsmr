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
use std::fmt;
use std::slice;
use std::time::Instant;

use allocative::Allocative;
use async_trait::async_trait;
use bsmr_artifact::artifact::build_artifact::BuildArtifact;
use bsmr_build_api::actions::Action;
use bsmr_build_api::actions::ActionExecutionCtx;
use bsmr_build_api::actions::UnregisteredAction;
use bsmr_build_api::actions::execute::action_executor::ActionExecutionKind;
use bsmr_build_api::actions::execute::action_executor::ActionExecutionMetadata;
use bsmr_build_api::actions::execute::action_executor::ActionOutputs;
use bsmr_build_api::actions::execute::error::ExecuteError;
use bsmr_build_api::actions::impls::json;
use bsmr_build_api::actions::impls::json::JsonUnpack;
use bsmr_build_api::actions::impls::json::validate_json;
use bsmr_build_api::artifact_groups::ArtifactGroup;
use bsmr_build_api::command_line_arg_like_impl;
use bsmr_build_api::interpreter::rule_defs::cmd_args::ArtifactPathMapper;
use bsmr_build_api::interpreter::rule_defs::cmd_args::CommandLineArgLike;
use bsmr_build_api::interpreter::rule_defs::cmd_args::CommandLineArtifactVisitor;
use bsmr_build_api::interpreter::rule_defs::cmd_args::CommandLineBuilder;
use bsmr_build_api::interpreter::rule_defs::cmd_args::WriteToFileMacroVisitor;
use bsmr_build_api::interpreter::rule_defs::cmd_args::value_as::ValueAsCommandLineLike;
use bsmr_build_signals::env::WaitingData;
use bsmr_common::file_ops::metadata::TrackedFileDigest;
use bsmr_core::category::CategoryRef;
use bsmr_core::content_hash::ContentBasedPathHash;
use bsmr_error::internal_error;
use bsmr_execute::artifact::fs::ExecutorFs;
use bsmr_execute::execute::command_executor::ActionExecutionTimingData;
use bsmr_execute::materialize::materializer::WriteRequest;
use bsmr_hash::BuckIndexMap;
use bsmr_hash::BuckIndexSet;
use bsmr_hash::buck_indexmap;
use dupe::Dupe;
use pagable::Pagable;
use pagable::pagable_typetag;
use starlark::any::ProvidesStaticType;
use starlark::coerce::Coerce;
use starlark::environment::GlobalsBuilder;
use starlark::starlark_complex_value;
use starlark::starlark_module;
use starlark::values::Demand;
use starlark::values::Freeze;
use starlark::values::NoSerialize;
use starlark::values::OwnedFrozenValue;
use starlark::values::StarlarkPagable;
use starlark::values::StarlarkValue;
use starlark::values::Trace;
use starlark::values::UnpackValue;
use starlark::values::Value;
use starlark::values::ValueLifetimeless;
use starlark::values::ValueLike;
use starlark::values::starlark_value;
use starlark::values::type_repr::StarlarkTypeRepr;

use crate::actions::impls::run::DepFilesPlaceholderArtifactPathMapper;
use crate::actions::impls::write::CommandLineContentBasedInputVisitor;

#[derive(Debug, bsmr_error::Error)]
#[bsmr(tag = Tier0)]
enum WriteJsonActionValidationError {
    #[error("WriteJsonAction received no outputs")]
    NoOutputs,
    #[error("WriteJsonAction received more than one output")]
    TooManyOutputs,
}

#[derive(Allocative, Debug, Pagable)]
pub(crate) struct UnregisteredWriteJsonAction {
    pretty: bool,
    absolute: bool,
    use_dep_files_placeholder_for_content_based_paths: bool,
}

impl UnregisteredWriteJsonAction {
    pub(crate) fn new(
        pretty: bool,
        absolute: bool,
        use_dep_files_placeholder_for_content_based_paths: bool,
    ) -> Self {
        Self {
            pretty,
            absolute,
            use_dep_files_placeholder_for_content_based_paths,
        }
    }

    pub(crate) fn cli<'v>(
        artifact: Value<'v>,
        content: Value<'v>,
    ) -> bsmr_error::Result<StarlarkWriteJsonCommandLineArg<'v>> {
        Ok(StarlarkWriteJsonCommandLineArg { artifact, content })
    }
}

impl UnregisteredAction for UnregisteredWriteJsonAction {
    fn register(
        self: Box<Self>,
        outputs: BuckIndexSet<BuildArtifact>,
        starlark_data: Option<OwnedFrozenValue>,
        _error_handler: Option<OwnedFrozenValue>,
    ) -> bsmr_error::Result<Box<dyn Action>> {
        let contents = starlark_data.expect("module data to be present");
        let action = WriteJsonAction::new(contents, outputs, *self)?;
        Ok(Box::new(action))
    }
}

#[derive(Debug, Allocative, Pagable)]
struct WriteJsonAction {
    contents: OwnedFrozenValue, // JSON value
    output: BuildArtifact,
    inner: UnregisteredWriteJsonAction,
}

impl WriteJsonAction {
    fn new(
        contents: OwnedFrozenValue,
        outputs: BuckIndexSet<BuildArtifact>,
        inner: UnregisteredWriteJsonAction,
    ) -> bsmr_error::Result<Self> {
        validate_json(JsonUnpack::unpack_value_err(contents.value())?)?;

        let mut outputs = outputs.into_iter();

        let output = match (outputs.next(), outputs.next()) {
            (Some(o), None) => o,
            (None, ..) => return Err(WriteJsonActionValidationError::NoOutputs.into()),
            (Some(..), Some(..)) => {
                return Err(WriteJsonActionValidationError::TooManyOutputs.into());
            }
        };

        Ok(WriteJsonAction {
            contents,
            output,
            inner,
        })
    }

    fn get_contents(
        &self,
        fs: &ExecutorFs,
        artifact_path_mapping: &dyn ArtifactPathMapper,
    ) -> bsmr_error::Result<Vec<u8>> {
        let mut writer = Vec::new();
        json::write_json(
            JsonUnpack::unpack_value_err(self.contents.value())?,
            Some(fs),
            &mut writer,
            self.inner.pretty,
            self.inner.absolute,
            artifact_path_mapping,
        )?;
        Ok(writer)
    }
}

#[pagable_typetag]
#[async_trait]
impl Action for WriteJsonAction {
    fn kind(&self) -> bsmr_data::ActionKind {
        bsmr_data::ActionKind::Write
    }

    fn inputs(&self) -> bsmr_error::Result<Cow<'_, [ArtifactGroup]>> {
        if self.inner.use_dep_files_placeholder_for_content_based_paths {
            return Ok(Cow::Borrowed(&[]));
        }

        let mut visitor = CommandLineContentBasedInputVisitor::new();
        json::visit_json_artifacts(self.contents.value(), &mut visitor)?;
        Ok(Cow::Owned(
            visitor.content_based_inputs.into_iter().collect(),
        ))
    }

    fn outputs(&self) -> Cow<'_, [BuildArtifact]> {
        Cow::Borrowed(slice::from_ref(&self.output))
    }

    fn first_output(&self) -> &BuildArtifact {
        &self.output
    }

    fn category(&self) -> CategoryRef<'_> {
        CategoryRef::unchecked_new("write_json")
    }

    fn identifier(&self) -> Option<&str> {
        Some(self.output.get_path().path().as_str())
    }

    fn aquery_attributes(
        &self,
        fs: &ExecutorFs,
        artifact_path_mapping: &dyn ArtifactPathMapper,
    ) -> BuckIndexMap<String, String> {
        let res: bsmr_error::Result<String> = try {
            let content = self.get_contents(fs, artifact_path_mapping)?;
            String::from_utf8(content).map_err(bsmr_error::Error::from)?
        };
        // TODO(cjhopman): We should change this api to support returning a Result.
        buck_indexmap! {
            "contents".to_owned() => match res {
                Ok(v) => v,
                Err(e) => format!("ERROR: constructing contents ({e})")
            },
            "absolute".to_owned() => self.inner.absolute.to_string(),
        }
    }

    async fn execute(
        &self,
        ctx: &mut dyn ActionExecutionCtx,
        waiting_data: WaitingData,
    ) -> Result<(ActionOutputs, ActionExecutionMetadata), ExecuteError> {
        let fs = ctx.fs();

        let mut execution_start = None;
        let value = ctx
            .materializer()
            .declare_write(Box::new(|| {
                execution_start = Some(Instant::now());
                let content = if self.inner.use_dep_files_placeholder_for_content_based_paths {
                    self.get_contents(
                        &ctx.executor_fs(),
                        &DepFilesPlaceholderArtifactPathMapper {},
                    )?
                } else {
                    self.get_contents(&ctx.executor_fs(), &ctx.artifact_path_mapping(None))?
                };
                let path = fs.resolve_build(
                    self.output.get_path(),
                    if self.output.get_path().is_content_based_path() {
                        let digest = TrackedFileDigest::from_content(
                            &content,
                            ctx.digest_config().cas_digest_config(),
                        );
                        Some(ContentBasedPathHash::new(digest.raw_digest().as_bytes())?)
                    } else {
                        None
                    }
                    .as_ref(),
                )?;
                let configuration_path = ctx
                    .materializer()
                    .maybe_eager_configuration_path(fs, self.output.get_path())?;
                Ok(vec![WriteRequest {
                    path,
                    content,
                    is_executable: false,
                    configuration_path,
                }])
            }))
            .await?
            .into_iter()
            .next()
            .ok_or_else(|| internal_error!("Write did not execute"))?;

        let wall_time = Instant::now()
            - execution_start
                .ok_or_else(|| internal_error!("Action did not set execution_start"))?;

        Ok((
            ActionOutputs::new(buck_indexmap![self.output.get_path().dupe() => value]),
            ActionExecutionMetadata {
                execution_kind: ActionExecutionKind::Simple,
                timing: ActionExecutionTimingData { wall_time },
                input_files_bytes: None,
                waiting_data,
            },
        ))
    }
}

/// WriteJsonCommandLineArgGen represents the artifact produced by write_json in a way that it can
/// be added to commandlines while including the artifacts referenced by cmdargs in the content that
/// was written.
#[derive(
    Debug,
    Clone,
    Trace,
    Coerce,
    Freeze,
    ProvidesStaticType,
    Allocative,
    StarlarkPagable
)]
#[derive(NoSerialize)] // TODO we should probably have a serialization for transitive set
#[repr(C)]
pub(crate) struct StarlarkWriteJsonCommandLineArgGen<V: ValueLifetimeless> {
    artifact: V,
    // The list of artifacts here could be large and we don't want to hold those explicitly (due to
    // the memory cost) and so we hold the same content value that the write_json action itself will and
    // only traverse it when artifacts are requested.
    content: V,
}

impl<'v, V: ValueLike<'v>> fmt::Display for StarlarkWriteJsonCommandLineArgGen<V> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<WriteJsonCliArgs>")
    }
}

starlark_complex_value!(pub(crate) StarlarkWriteJsonCommandLineArg);

#[starlark_value(type = "WriteJsonCliArgs")]
impl<'v, V: ValueLike<'v>> StarlarkValue<'v> for StarlarkWriteJsonCommandLineArgGen<V>
where
    Self: ProvidesStaticType<'v>,
{
    fn provide(&'v self, demand: &mut Demand<'_, 'v>) {
        demand.provide_value::<&dyn CommandLineArgLike>(self);
    }
}

impl<'v, V: ValueLike<'v>> StarlarkWriteJsonCommandLineArgGen<V> {
    pub fn visit_contents(
        &self,
        visitor: &mut dyn CommandLineArtifactVisitor<'v>,
    ) -> bsmr_error::Result<()> {
        let content = self.content.to_value();
        json::visit_json_artifacts(content, visitor)
    }
}

impl<'v, V: ValueLike<'v>> CommandLineArgLike<'v> for StarlarkWriteJsonCommandLineArgGen<V> {
    fn register_me(&self) {
        command_line_arg_like_impl!(StarlarkWriteJsonCommandLineArg::starlark_type_repr());
    }

    fn add_to_command_line(&self, fmt: &mut CommandLineBuilder<'v, '_>) -> bsmr_error::Result<()> {
        ValueAsCommandLineLike::unpack_value_err(self.artifact.to_value())?
            .0
            .add_to_command_line(fmt)
    }

    fn visit_artifacts(
        &self,
        visitor: &mut dyn CommandLineArtifactVisitor<'v>,
    ) -> bsmr_error::Result<()> {
        let artifact = self.artifact.to_value();
        let content = self.content.to_value();
        ValueAsCommandLineLike::unpack_value_err(artifact)?
            .0
            .visit_artifacts(visitor)?;
        json::visit_json_artifacts(content, visitor)
    }

    fn contains_arg_attr(&self) -> bool {
        // In the write_json implementation, the CommandLineBuilder we use don't support args.
        false
    }

    fn visit_write_to_file_macros(
        &self,
        _visitor: &mut dyn WriteToFileMacroVisitor,
        _artifact_path_mapping: &dyn ArtifactPathMapper,
    ) -> bsmr_error::Result<()> {
        // In the write_json implementation, the CommandLineBuilder we use don't support args.
        Ok(())
    }
}

#[starlark_module]
#[starlark_types(
    StarlarkWriteJsonCommandLineArg<'_> as WriteJsonCliArgs
)]
pub(crate) fn register_write_json_cli_args(globals: &mut GlobalsBuilder) {}
