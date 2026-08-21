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

import com.dedalus.bsmr.cd.model.java.AbiGenerationMode;
import com.dedalus.bsmr.core.build.execution.context.IsolatedExecutionContext;
import com.dedalus.bsmr.core.filesystems.AbsPath;
import com.dedalus.bsmr.core.filesystems.RelPath;
import com.dedalus.bsmr.io.filesystem.impl.ProjectFilesystemUtils;
import com.dedalus.bsmr.jvm.java.abi.StubJar;
import com.dedalus.bsmr.step.StepExecutionResult;
import com.dedalus.bsmr.step.StepExecutionResults;
import com.dedalus.bsmr.step.isolatedsteps.IsolatedStep;
import com.google.common.base.Preconditions;
import java.io.IOException;
import java.nio.file.Path;
import javax.annotation.Nullable;

/** Calculates class abi from the library.jar */
public class CalculateClassAbiStep implements IsolatedStep {

  private final RelPath binaryJar;
  @Nullable private final RelPath existingAbiJar;
  private final RelPath abiJar;
  private final AbiGenerationMode compatibilityMode;

  public CalculateClassAbiStep(
      RelPath binaryJar,
      @Nullable RelPath existingAbiJar,
      RelPath abiJar,
      AbiGenerationMode compatibilityMode) {
    this.binaryJar = binaryJar;
    this.existingAbiJar = existingAbiJar;
    this.abiJar = abiJar;
    this.compatibilityMode = compatibilityMode;
  }

  @Override
  public StepExecutionResult executeIsolatedStep(IsolatedExecutionContext context)
      throws IOException {
    AbsPath ruleCellRoot = context.getRuleCellRoot();
    AbsPath output = toAbsOutputPath(ruleCellRoot, abiJar);

    try {
      StubJar stubJar =
          new StubJar(ruleCellRoot.resolve(binaryJar), false)
              .setCompatibilityMode(compatibilityMode);
      if (existingAbiJar != null
          && ProjectFilesystemUtils.exists(ruleCellRoot, existingAbiJar.getPath())) {
        stubJar = stubJar.setExistingAbiJar(ruleCellRoot.resolve(existingAbiJar));
      }
      stubJar.writeTo(output);
    } catch (IllegalArgumentException e) {
      throw new RuntimeException(String.format("Failed to calculate ABI for %s.", binaryJar), e);
    }

    return StepExecutionResults.SUCCESS;
  }

  private AbsPath toAbsOutputPath(AbsPath root, RelPath relativeOutputPath) throws IOException {
    Path outputPath = ProjectFilesystemUtils.getPathForRelativePath(root, relativeOutputPath);
    Preconditions.checkState(
        !ProjectFilesystemUtils.exists(root, outputPath),
        "Output file already exists: %s",
        relativeOutputPath);

    if (outputPath.getParent() != null
        && !ProjectFilesystemUtils.exists(root, outputPath.getParent())) {
      ProjectFilesystemUtils.createParentDirs(root, outputPath);
    }
    return root.resolve(relativeOutputPath);
  }

  @Override
  public String getShortName() {
    return "class_abi";
  }

  @Override
  public String getIsolatedStepDescription(IsolatedExecutionContext context) {
    return String.format("%s %s", getShortName(), binaryJar);
  }
}
