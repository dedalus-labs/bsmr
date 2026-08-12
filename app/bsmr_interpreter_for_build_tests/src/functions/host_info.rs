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

use bsmr_interpreter_for_build::interpreter::testing::Tester;
use indoc::indoc;

#[test]
fn test_host_info() -> bsmr_error::Result<()> {
    let mut tester = Tester::new().unwrap();
    tester.run_starlark_test(indoc!(
        r#"
            def test():
                assert_eq(True, host_info().os.is_linux)
                assert_eq(False, host_info().os.is_macos)
                assert_eq(False, host_info().os.is_macos)

                assert_eq(True, host_info().arch.is_x86_64)
                assert_eq(False, host_info().arch.is_arm)
                assert_eq(False, host_info().arch.is_mipsel64)

            "#
    ))?;
    Ok(())
}

#[test]
fn test_buck_v2() -> bsmr_error::Result<()> {
    let mut tester = Tester::new().unwrap();
    tester.run_starlark_test(indoc!(
        r#"
            def test():
                assert_eq(True, hasattr(host_info(), "bsmr"))
                assert_eq(False, hasattr(host_info(), "buck1"))
        "#
    ))?;
    Ok(())
}
