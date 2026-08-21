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

package com.dedalus.bsmr.testrunner.reportlayer;

import com.dedalus.bsmr.testrunner.InstrumentationTestRunner;

/**
 * base class for report layers for InstrumentationTestRunner. report layers like video recording,
 * tombstone reporting will use it as base class
 */
public abstract class ReportLayer {
  protected final InstrumentationTestRunner runner;

  public ReportLayer(InstrumentationTestRunner runner) {
    this.runner = runner;
  }

  /*
  initialize the report layer
   */
  public abstract void initialize();

  /*
  do the actual reporting
   */
  public abstract void report();
}
