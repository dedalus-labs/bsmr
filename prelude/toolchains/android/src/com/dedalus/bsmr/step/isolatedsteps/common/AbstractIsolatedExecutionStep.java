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

package com.dedalus.bsmr.step.isolatedsteps.common;

import com.dedalus.bsmr.core.build.execution.context.IsolatedExecutionContext;
import com.dedalus.bsmr.step.StepExecutionResult;
import com.dedalus.bsmr.step.isolatedsteps.IsolatedStep;
import java.io.IOException;

/**
 * Abstract implementation of {@link IsolatedStep} that takes the description as a constructor
 * parameter and requires only the implementation of {@link
 * #executeIsolatedStep(IsolatedExecutionContext)}. This facilitates the creation of an anonymous
 * implementation of {@link IsolatedStep}.
 */
public abstract class AbstractIsolatedExecutionStep implements IsolatedStep {

  private final String description;

  public AbstractIsolatedExecutionStep(String description) {
    this.description = description;
  }

  @Override
  public abstract StepExecutionResult executeIsolatedStep(IsolatedExecutionContext context)
      throws IOException, InterruptedException;

  @Override
  public String getShortName() {
    return description;
  }

  @Override
  public String getIsolatedStepDescription(IsolatedExecutionContext context) {
    return description;
  }
}
