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

use bsmr_build_api::interpreter::rule_defs::cmd_args::CommandLineBuilder;
use bsmr_build_api::interpreter::rule_defs::cmd_args::SimpleCommandLineArtifactVisitor;
use bsmr_build_api::interpreter::rule_defs::cmd_args::StarlarkCommandLineInputs;
use bsmr_build_api::interpreter::rule_defs::cmd_args::value_as::ValueAsCommandLineLike;
use bsmr_build_api::interpreter::rule_defs::register_rule_defs;
use bsmr_core::execution_types::executor_config::PathSeparatorKind;
use bsmr_core::fs::artifact_path_resolver::ArtifactFs;
use bsmr_core::fs::output_path::OutputPathResolver;
use bsmr_core::fs::project::ProjectRoot;
use bsmr_core::fs::project_rel_path::ProjectRelativePathBuf;
use bsmr_execute::artifact::fs::ExecutorFs;
use bsmr_fs::paths::abs_norm_path::AbsNormPathBuf;
use bsmr_hash::BsmrHashMap;
use bsmr_interpreter::types::regex::register_bsmr_regex;
use bsmr_interpreter_for_build::interpreter::testing::Tester;
use bsmr_interpreter_for_build::interpreter::testing::cells;
use bsmr_interpreter_for_build::label::testing::label_creator;
use starlark::environment::GlobalsBuilder;
use starlark::starlark_module;
use starlark::values::UnpackValue;
use starlark::values::Value;
use starlark::values::list_or_tuple::UnpackListOrTuple;

use crate::interpreter::rule_defs::artifact::testing::artifactory;

fn artifact_fs() -> ArtifactFs {
    let cell_info = cells(None).unwrap();
    ArtifactFs::new(
        cell_info.1,
        OutputPathResolver::new(ProjectRelativePathBuf::unchecked_new(
            "bsmr-out/default".to_owned(),
        )),
        ProjectRoot::new(AbsNormPathBuf::try_from(std::env::current_dir().unwrap()).unwrap())
            .unwrap(),
    )
}

fn get_command_line(value: Value) -> bsmr_error::Result<Vec<String>> {
    let fs = artifact_fs();
    let executor_fs = ExecutorFs::new(&fs, PathSeparatorKind::Unix);
    let mut cli = Vec::<String>::new();
    let artifact_path_mapping = BsmrHashMap::default();
    let mut fmt = CommandLineBuilder::new(&mut cli, &artifact_path_mapping, &executor_fs);

    match ValueAsCommandLineLike::unpack_value(value)? {
        Some(v) => v.0.add_to_command_line(&mut fmt),
        None => ValueAsCommandLineLike::unpack_value_err(value)?
            .0
            .add_to_command_line(&mut fmt),
    }?;
    Ok(cli)
}

#[starlark_module]
pub(crate) fn command_line_stringifier(builder: &mut GlobalsBuilder) {
    fn get_args<'v>(value: Value<'v>) -> starlark::Result<Vec<String>> {
        Ok(get_command_line(value)?)
    }

    fn stringify_cli_arg<'v>(value: Value<'v>) -> starlark::Result<String> {
        let fs = artifact_fs();
        let executor_fs = ExecutorFs::new(&fs, PathSeparatorKind::Unix);
        let mut cli = Vec::<String>::new();
        let artifact_path_mapping = BsmrHashMap::default();
        let mut fmt = CommandLineBuilder::new(&mut cli, &artifact_path_mapping, &executor_fs);
        ValueAsCommandLineLike::unpack_value_err(value)?
            .0
            .add_to_command_line(&mut fmt)?;
        assert_eq!(1, cli.len());
        Ok(cli.first().unwrap().clone())
    }
}

#[starlark_module]
pub(crate) fn inputs_helper(builder: &mut GlobalsBuilder) {
    fn make_inputs<'v>(
        values: UnpackListOrTuple<Value<'v>>,
    ) -> starlark::Result<StarlarkCommandLineInputs> {
        let mut visitor = SimpleCommandLineArtifactVisitor::new();
        for v in values {
            let cli = ValueAsCommandLineLike::unpack_value_err(v)?.0;
            cli.visit_artifacts(&mut visitor)?;
        }

        Ok(StarlarkCommandLineInputs {
            inputs: visitor.inputs,
        })
    }
}

pub(crate) fn tester() -> bsmr_error::Result<Tester> {
    let mut tester = Tester::new()?;
    tester.additional_globals(command_line_stringifier);
    tester.additional_globals(inputs_helper);
    tester.additional_globals(artifactory);
    tester.additional_globals(label_creator);
    tester.additional_globals(register_rule_defs);
    tester.additional_globals(register_bsmr_regex);
    Ok(tester)
}
