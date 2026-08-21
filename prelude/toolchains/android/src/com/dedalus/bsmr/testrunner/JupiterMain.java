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

package com.dedalus.bsmr.testrunner;

import static com.dedalus.bsmr.testrunner.CheckDependency.classPresent;
import static com.dedalus.bsmr.testrunner.CheckDependency.requiresClass;

/**
 * Launcher for JUnit Jupiter (Junit5).
 *
 * @see JUnitMain
 */
public class JupiterMain {

  private JupiterMain() {
    // Launcher class.
  }

  /**
   * JUnit5 (Jupiter) will require its engine and launcher to be present into the classpath. In case
   * JUnit4 is also present it will require its vintage-engine to be present into the classpath.
   *
   * @param args runner arguments.
   */
  public static void main(String[] args) {
    // Requires Jupiter (JUnit5) engine + platform + launcher
    requiresClass("junit-jupiter-api", "org.junit.jupiter.api.Test");
    requiresClass("junit-jupiter-engine", "org.junit.jupiter.engine.JupiterTestEngine");
    requiresClass("junit-jupiter-platform", "org.junit.platform.engine.TestEngine");
    requiresClass(
        "junit-platform-launcher", "org.junit.platform.launcher.LauncherDiscoveryRequest");
    // If JUnit4 api is also present, it will require Vintage Engine to run
    if (classPresent("org.junit.Test")) {
      requiresClass("junit-vintage-engine", "org.junit.vintage.engine.VintageTestEngine");
    }

    JupiterRunner runner = new JupiterRunner();
    runner.parseArgs(args);
    runner.runAndExit();
  }
}
