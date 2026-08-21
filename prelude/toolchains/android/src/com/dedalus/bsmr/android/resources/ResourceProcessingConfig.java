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

package com.dedalus.bsmr.android.resources;

import com.facebook.infer.annotation.Nullsafe;

/**
 * Controls whether optimized code paths are used in the Android resource processing pipeline.
 *
 * <p>When disabled (the default), original code paths are used. When enabled, optimized code paths
 * are used for improved performance. This allows A/B testing of optimizations via:
 *
 * <pre>bsmr build ig4a -c android.optimized_resource_processing=true</pre>
 */
@Nullsafe(Nullsafe.Mode.LOCAL)
public class ResourceProcessingConfig {
  private static volatile boolean optimizationsEnabled = false;

  private ResourceProcessingConfig() {}

  public static void setOptimizationsEnabled(boolean enabled) {
    optimizationsEnabled = enabled;
  }

  public static boolean areOptimizationsEnabled() {
    return optimizationsEnabled;
  }
}
