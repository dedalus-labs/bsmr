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

use std::fmt::Display;
use std::fmt::Formatter;

use bsmr_core::fs::project_rel_path::ProjectRelativePath;
use dupe::Dupe;
use itertools::Itertools;

#[derive(Copy, Clone, Dupe)]
pub(crate) enum MaterializeStack<'a> {
    Empty,
    Child(&'a MaterializeStack<'a>, &'a ProjectRelativePath),
}

impl Display for MaterializeStack<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        if let MaterializeStack::Empty = self {
            return write!(f, "(empty)");
        }

        // Avoid recursion because we are fighting with stack overflow here,
        // and we do not want another stack overflow when producing error message.
        let mut stack = Vec::new();
        let mut current = *self;
        while let MaterializeStack::Child(parent, path) = current {
            stack.push(path);
            current = *parent;
        }
        write!(f, "{}", stack.iter().rev().join(" -> "))
    }
}

#[test]
fn test_materialize_stack_display() {
    let s = MaterializeStack::Empty;
    assert_eq!("(empty)", s.to_string());
    let s = MaterializeStack::Child(&s, ProjectRelativePath::new("foo").unwrap());
    assert_eq!("foo", s.to_string());
    let s = MaterializeStack::Child(&s, ProjectRelativePath::new("bar/baz").unwrap());
    assert_eq!("foo -> bar/baz", s.to_string());
}
