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

package com.dedalus.bsmr.step;

import com.dedalus.bsmr.core.build.execution.context.IsolatedExecutionContext;
import com.dedalus.bsmr.core.filesystems.AbsPath;
import com.dedalus.bsmr.util.ClassLoaderCache;
import com.dedalus.bsmr.util.Console;

public class TestExecutionContext {

  private TestExecutionContext() {
    // Utility class.
  }

  // For test code, use a global class loader cache to avoid having to call ExecutionContext.close()
  // in each test case.
  private static final ClassLoaderCache testClassLoaderCache = new ClassLoaderCache();

  public static IsolatedExecutionContext newInstance(AbsPath rootPath) {
    return new IsolatedExecutionContext(
        testClassLoaderCache, Console.createNullConsole(), rootPath);
  }
}
