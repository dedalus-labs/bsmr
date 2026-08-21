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

use bsmr_artifact::actions::key::ActionIndex;
use bsmr_artifact::artifact::artifact_type::testing::ArtifactTestingExt;
use bsmr_artifact::artifact::artifact_type::testing::BuildArtifactTestingExt;
use bsmr_artifact::artifact::build_artifact::BuildArtifact;
use bsmr_build_api::actions::ActionErrors;
use bsmr_build_api::actions::registry::ActionsRegistry;
use bsmr_build_api::analysis::registry::AnalysisValueFetcher;
use bsmr_build_api::artifact_groups::ArtifactGroup;
use bsmr_core::category::Category;
use bsmr_core::category::CategoryRef;
use bsmr_core::configuration::data::ConfigurationData;
use bsmr_core::configuration::pair::ConfigurationNoExec;
use bsmr_core::deferred::base_deferred_key::BaseDeferredKey;
use bsmr_core::deferred::key::DeferredHolderKey;
use bsmr_core::execution_types::execution::ExecutionPlatform;
use bsmr_core::execution_types::execution::ExecutionPlatformResolution;
use bsmr_core::execution_types::executor_config::CommandExecutorConfig;
use bsmr_core::fs::output_path::BuildArtifactPath;
use bsmr_core::fs::output_path::OutputPathKind;
use bsmr_core::target::configured_target_label::ConfiguredTargetLabel;
use bsmr_execute::execute::request::OutputType;
use bsmr_fs::paths::forward_rel_path::ForwardRelativePathBuf;
use bsmr_hash::bsmr_indexset;
use dupe::Dupe;
use itertools::Itertools;
use starlark::values::Heap;

use crate::actions::testings::SimpleUnregisteredAction;

#[test]
fn declaring_artifacts() -> bsmr_error::Result<()> {
    Heap::temp(|heap| {
        let base = BaseDeferredKey::TargetLabel(ConfiguredTargetLabel::testing_parse(
            "cell//pkg:foo",
            ConfigurationData::testing_new(),
        ));
        let mut actions = ActionsRegistry::new(
            DeferredHolderKey::Base(base.dupe()),
            ExecutionPlatformResolution::unspecified(),
        );
        let out1 = ForwardRelativePathBuf::unchecked_new("bar.out".into());
        let bsmrout1 = BuildArtifactPath::new(base.dupe(), out1.clone(), OutputPathKind::default());
        let declared1 = actions.declare_artifact(
            None,
            out1.clone(),
            OutputType::File,
            None,
            OutputPathKind::default(),
            heap,
        )?;
        declared1
            .get_path()
            .with_full_path(|p| assert_eq!(p, bsmrout1.path()));

        let out2 = ForwardRelativePathBuf::unchecked_new("bar2.out".into());
        let bsmrout2 = BuildArtifactPath::new(base, out2.clone(), OutputPathKind::default());
        let declared2 = actions.declare_artifact(
            None,
            out2,
            OutputType::File,
            None,
            OutputPathKind::default(),
            heap,
        )?;
        declared2
            .get_path()
            .with_full_path(|p| assert_eq!(p, bsmrout2.path()));

        if actions
            .declare_artifact(
                None,
                out1,
                OutputType::File,
                None,
                OutputPathKind::default(),
                heap,
            )
            .is_ok()
        {
            panic!("should error due to duplicate artifact")
        }

        assert!(actions.testing_artifacts().contains(&declared1));
        assert!(actions.testing_artifacts().contains(&declared2));

        Ok(())
    })
}

#[test]
fn claiming_conflicting_path() -> bsmr_error::Result<()> {
    let mut actions = ActionsRegistry::new(
        DeferredHolderKey::testing_new("cell//pkg:my_target"),
        ExecutionPlatformResolution::unspecified(),
    );

    let out1 = ForwardRelativePathBuf::unchecked_new("foo/a/1".into());
    actions.claim_output_path(&out1, None)?;

    let out2 = ForwardRelativePathBuf::unchecked_new("foo/a/2".into());
    actions.claim_output_path(&out2, None)?;

    {
        let expected_conflicts = vec!["foo/a/1 declared at <unknown>".to_owned()];
        let prefix_claimed = ForwardRelativePathBuf::unchecked_new("foo/a/1/some/path".into());

        let actual = actions
            .claim_output_path(&prefix_claimed, None)
            .unwrap_err();
        let expected: bsmr_error::Error =
            ActionErrors::ConflictingOutputPaths(prefix_claimed, expected_conflicts).into();
        assert_eq!(actual.to_string(), expected.to_string());
    }

    let err = actions.claim_output_path(&out1, None).unwrap_err();
    assert!(
        err.category_key()
            .ends_with("ActionErrors::ConflictingOutputPath")
    );

    {
        let overwrite_dir = ForwardRelativePathBuf::unchecked_new("foo".into());
        let expected_conflicts = vec![
            "foo/a/1 declared at <unknown>".to_owned(),
            "foo/a/2 declared at <unknown>".to_owned(),
        ];

        let actual = actions.claim_output_path(&overwrite_dir, None).unwrap_err();
        let expected: bsmr_error::Error =
            ActionErrors::ConflictingOutputPaths(overwrite_dir, expected_conflicts).into();
        assert_eq!(actual.to_string(), expected.to_string());
    }

    Ok(())
}

#[test]
fn register_actions() -> bsmr_error::Result<()> {
    Heap::temp(|heap| {
        let base = BaseDeferredKey::TargetLabel(ConfiguredTargetLabel::testing_parse(
            "cell//pkg:foo",
            ConfigurationData::testing_new(),
        ));
        let mut actions = ActionsRegistry::new(
            DeferredHolderKey::Base(base.dupe()),
            ExecutionPlatformResolution::unspecified(),
        );
        let out = ForwardRelativePathBuf::unchecked_new("bar.out".into());
        let declared = actions.declare_artifact(
            None,
            out,
            OutputType::File,
            None,
            OutputPathKind::default(),
            heap,
        )?;

        let inputs = bsmr_indexset![ArtifactGroup::Artifact(
            BuildArtifact::testing_new(
                base.unpack_target_label().unwrap().dupe(),
                "input",
                ActionIndex::new(1),
            )
            .into()
        )];
        let outputs = bsmr_indexset![declared.as_output()];

        let unregistered_action = SimpleUnregisteredAction::new(
            inputs,
            vec![],
            CategoryRef::new("fake_action").unwrap().to_owned(),
            None,
        );

        let key = actions.register(
            &DeferredHolderKey::Base(base.dupe()),
            outputs,
            unregistered_action.clone(),
        )?;

        assert_eq!(actions.testing_pending_action_keys(), vec![key]);
        assert!(declared.testing_is_bound());

        Ok(())
    })
}

#[test]
fn finalizing_actions() -> bsmr_error::Result<()> {
    Heap::temp(|heap| {
        let base = BaseDeferredKey::TargetLabel(ConfiguredTargetLabel::testing_parse(
            "cell//pkg:foo",
            ConfigurationData::testing_new(),
        ));
        let mut actions = ActionsRegistry::new(
            DeferredHolderKey::Base(base.dupe()),
            ExecutionPlatformResolution::new_for_testing(
                Some(ExecutionPlatform::legacy_execution_platform(
                    CommandExecutorConfig::testing_local(),
                    ConfigurationNoExec::testing_new(),
                )),
                Vec::new(),
            ),
        );
        let out = ForwardRelativePathBuf::unchecked_new("bar.out".into());
        let declared = actions.declare_artifact(
            None,
            out,
            OutputType::File,
            None,
            OutputPathKind::default(),
            heap,
        )?;

        let inputs = bsmr_indexset![ArtifactGroup::Artifact(
            BuildArtifact::testing_new(
                base.unpack_target_label().unwrap().dupe(),
                "input",
                ActionIndex::new(1),
            )
            .into()
        )];
        let outputs = bsmr_indexset![declared.as_output()];

        let unregistered_action = SimpleUnregisteredAction::new(
            inputs,
            vec![],
            CategoryRef::new("fake_action").unwrap().to_owned(),
            None,
        );
        let holder_key = DeferredHolderKey::Base(base.dupe());
        actions.register(&holder_key, outputs, unregistered_action)?;

        let result = (actions.finalize()?)(&AnalysisValueFetcher::testing_new(holder_key))?;

        assert!(
            result
                .lookup(&declared.testing_action_key().unwrap())
                .is_ok(),
            "Expected results to contain `{}`, had `[{}]`",
            declared.testing_action_key().unwrap(),
            result.iter_actions().map(|v| v.key()).join(", ")
        );

        Ok(())
    })
}

#[test]
fn duplicate_category_singleton_actions() {
    let result =
        category_identifier_test(&[("singleton_category", None), ("singleton_category", None)])
            .unwrap_err();

    assert!(
        result
            .category_key()
            .ends_with("ActionErrors::ActionCategoryDuplicateSingleton")
    );
}

#[test]
fn duplicate_category_identifier() {
    let result = category_identifier_test(&[
        ("cxx_compile", Some("foo.cpp")),
        ("cxx_compile", Some("foo.cpp")),
    ])
    .unwrap_err();

    assert!(
        result
            .category_key()
            .ends_with("ActionErrors::ActionCategoryIdentifierNotUnique")
    );
}

fn category_identifier_test(
    action_names: &[(&'static str, Option<&'static str>)],
) -> bsmr_error::Result<()> {
    let base = DeferredHolderKey::testing_new("cell//pkg:foo");
    let mut actions = ActionsRegistry::new(
        base.dupe(),
        ExecutionPlatformResolution::new_for_testing(
            Some(ExecutionPlatform::legacy_execution_platform(
                CommandExecutorConfig::testing_local(),
                ConfigurationNoExec::testing_new(),
            )),
            Vec::new(),
        ),
    );
    for (category, identifier) in action_names {
        let unregistered_action = SimpleUnregisteredAction::new(
            bsmr_indexset![],
            vec![],
            Category::new((*category).to_owned()).unwrap(),
            identifier.map(|i| i.to_owned()),
        );

        actions.register(&base, bsmr_indexset![], unregistered_action)?;
    }

    (actions.finalize()?)(&AnalysisValueFetcher::testing_new(base))?;
    Ok(())
}
