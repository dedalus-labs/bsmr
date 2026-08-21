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
import com.dedalus.bsmr.util.environment.EnvVariablesProvider;
import com.facebook.infer.annotation.Nullsafe;
import com.google.common.collect.ImmutableMap;

@Nullsafe(Nullsafe.Mode.LOCAL)
public class AdbExecutionContext {
  private final Console console;

  public AdbExecutionContext(Console console) {
    this.console = console;
  }

  public ImmutableMap<String, String> getEnvironment() {
    return EnvVariablesProvider.getSystemEnv();
  }

  public Console getConsole() {
    return console;
  }
}
