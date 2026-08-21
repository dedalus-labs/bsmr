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

package com.dedalus.bsmr.android;

import com.dedalus.bsmr.util.Console;
import com.google.common.collect.ImmutableMap;

public class TestAdbExecutionContext extends AdbExecutionContext {

  private final ImmutableMap<String, String> extraEnvironment;

  public TestAdbExecutionContext(Console console) {
    this(console, ImmutableMap.of());
  }

  public TestAdbExecutionContext(Console console, ImmutableMap<String, String> environment) {
    super(console);
    extraEnvironment = environment;
  }

  @Override
  public ImmutableMap<String, String> getEnvironment() {
    return ImmutableMap.<String, String>builder()
        .putAll(super.getEnvironment())
        .putAll(extraEnvironment)
        .build();
  }
}
