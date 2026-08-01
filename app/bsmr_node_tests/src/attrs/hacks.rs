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

use bsmr_core::cells::name::CellName;
use bsmr_core::cells::paths::CellRelativePath;
use bsmr_core::package::PackageLabel;
use bsmr_fs::paths::forward_rel_path::ForwardRelativePath;
use bsmr_node::attrs::attr_type::list::ListLiteral;
use bsmr_node::attrs::attr_type::string::StringLiteral;
use bsmr_node::attrs::coerced_attr::CoercedAttr;
use bsmr_node::attrs::hacks;
use bsmr_util::arc_str::ArcSlice;
use bsmr_util::arc_str::ArcStr;
use dupe::Dupe;

#[test]
fn stringifies_correctly() -> bsmr_error::Result<()> {
    let coerced = CoercedAttr::String(StringLiteral(ArcStr::from("Hello, world!")));

    let package = PackageLabel::new(
        CellName::testing_new("root"),
        CellRelativePath::new(ForwardRelativePath::new("foo/bar").unwrap()),
    )?;

    assert_eq!(
        "Hello, world!".to_owned(),
        hacks::value_to_string(&coerced, package.dupe())?
    );

    let list = CoercedAttr::List(ListLiteral(ArcSlice::new([CoercedAttr::String(
        StringLiteral(ArcStr::from("Hello, world!")),
    )])));
    assert!(hacks::value_to_string(&list, package.dupe()).is_err());
    Ok(())
}
