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

use bsmr_build_api::interpreter::rule_defs::register_rule_defs;
use bsmr_build_api::interpreter::rule_defs::validation_spec;
use bsmr_interpreter_for_build::interpreter::testing::Tester;
use bsmr_interpreter_for_build::interpreter::testing::expect_error;
use indoc::indoc;

use crate::interpreter::rule_defs::artifact::testing::artifactory;

fn new_tester() -> Tester {
    let mut tester = Tester::new().unwrap();
    tester.additional_globals(register_rule_defs);
    tester.additional_globals(validation_spec::register_validation_spec);
    tester.additional_globals(artifactory);
    tester
}

#[test]
fn test_construction() -> bsmr_error::Result<()> {
    let mut tester = new_tester();
    let test = indoc!(
        r#"
        def test():
            a = declared_bound_artifact("//foo:bar", "baz/quz.h")
            ValidationInfo(validations=[ValidationSpec(name="foo", validation_result=a)])
        "#
    );
    tester.run_starlark_bzl_test(test)?;
    Ok(())
}

#[test]
fn test_missing_fields_validation() -> bsmr_error::Result<()> {
    let mut tester = new_tester();
    {
        let test = indoc!(
            r#"
            def test():
                ValidationInfo()
            "#
        );
        expect_error(
            tester.run_starlark_bzl_test(test),
            test,
            "Missing required parameter `validations`",
        );
    }
    Ok(())
}

#[test]
fn test_validation_failure() -> bsmr_error::Result<()> {
    let mut tester = new_tester();
    {
        let test = indoc!(
            r#"
            def test():
                ValidationInfo(validations=[1, 2])
            "#
        );
        expect_error(
            tester.run_starlark_bzl_test(test),
            test,
            "Expected type `list[ValidationSpec]` but got `list[int]`",
        );
    }
    {
        let test = indoc!(
            r#"
            def test():
                a = declared_bound_artifact("//foo:bar", "baz/quz.h")
                ValidationInfo(validations=[ValidationSpec(name="foo", validation_result=a), ValidationSpec(name="foo", validation_result=a)])
            "#
        );
        expect_error(
            tester.run_starlark_bzl_test(test),
            test,
            "Multiple specs with same name `foo` which is not allowed.",
        );
    }
    {
        let test = indoc!(
            r#"
            def test():
                ValidationInfo(validations=[])
            "#
        );
        expect_error(
            tester.run_starlark_bzl_test(test),
            test,
            "`ValidationInfo` should contain at least one validation.",
        );
    }
    Ok(())
}
