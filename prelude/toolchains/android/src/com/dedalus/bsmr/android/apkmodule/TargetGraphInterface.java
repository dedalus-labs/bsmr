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

package com.dedalus.bsmr.android.apkmodule;

import com.facebook.infer.annotation.Nullsafe;

/**
 * Interface for a directed graph of targets that can be traversed by getting the dependencies of
 * each node.
 */
@Nullsafe(Nullsafe.Mode.LOCAL)
public interface TargetGraphInterface<BuildTarget> {
  boolean isEmpty();

  HasBuildTargetAndBuildDeps<BuildTarget> get(BuildTarget buildTarget);
}
