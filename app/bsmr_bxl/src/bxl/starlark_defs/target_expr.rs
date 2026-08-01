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

use bsmr_node::nodes::frontend::TargetGraphCalculation;
use bsmr_node::nodes::unconfigured::TargetNode;
use bsmr_query::query::environment::QueryTarget;
use dice::DiceComputations;
use dupe::Dupe;

#[derive(Clone)]
pub(crate) enum TargetExpr<'v, Node: QueryTarget> {
    Node(Node),
    Label(Cow<'v, Node::Key>),
}

impl<'v, Node: QueryTarget> TargetExpr<'v, Node> {
    pub(crate) fn node_ref(&self) -> &Node::Key {
        match self {
            TargetExpr::Node(node) => node.node_key(),
            TargetExpr::Label(label) => label,
        }
    }
}

impl<'v> TargetExpr<'v, TargetNode> {
    pub(crate) async fn get_from_dice(
        &self,
        dice: &mut DiceComputations<'_>,
    ) -> bsmr_error::Result<TargetNode> {
        match self {
            TargetExpr::Node(node) => Ok(node.dupe()),
            TargetExpr::Label(label) => Ok(dice.get_target_node(label).await?),
        }
    }
}
