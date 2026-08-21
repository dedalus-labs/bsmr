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

package com.dedalus.bsmr.jvm.java;

import com.dedalus.bsmr.core.filesystems.RelPath;
import com.dedalus.bsmr.step.isolatedsteps.IsolatedStep;
import com.dedalus.bsmr.step.isolatedsteps.common.MkdirIsolatedStep;
import com.dedalus.bsmr.step.isolatedsteps.common.SymlinkIsolatedStep;
import com.google.common.collect.ImmutableList;
import com.google.common.collect.ImmutableMap;
import java.util.Map;

/** Copies (by creating symlinks) resources from existing paths to desired paths. */
public class CopyResourcesStep {

  private CopyResourcesStep() {}

  /** Copies (by creating symlinks) resources from existing paths to desired paths. */
  public static ImmutableList<IsolatedStep> of(ImmutableMap<RelPath, RelPath> resources) {
    ImmutableList.Builder<IsolatedStep> steps =
        ImmutableList.builderWithExpectedSize(resources.size() * 2);
    for (Map.Entry<RelPath, RelPath> entry : resources.entrySet()) {
      RelPath existingPath = entry.getKey();
      RelPath linkPath = entry.getValue();

      steps.add(new MkdirIsolatedStep(linkPath.getParent()));
      steps.add(new SymlinkIsolatedStep(existingPath, linkPath));
    }
    return steps.build();
  }
}
