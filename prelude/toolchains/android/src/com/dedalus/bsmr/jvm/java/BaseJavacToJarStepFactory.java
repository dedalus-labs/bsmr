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

import com.dedalus.bsmr.core.filesystems.AbsPath;
import com.dedalus.bsmr.core.filesystems.RelPath;
import com.dedalus.bsmr.jvm.core.BuildTargetValue;
import com.dedalus.bsmr.step.isolatedsteps.IsolatedStep;
import com.dedalus.bsmr.step.isolatedsteps.common.MkdirIsolatedStep;
import com.google.common.collect.ImmutableList;
import javax.annotation.Nullable;

/**
 * Java implementation of compile to jar steps factory that doesn't depend on internal build graph
 * datastructures, and only knows how to create compile steps.
 */
public class BaseJavacToJarStepFactory extends BaseCompileToJarStepFactory<JavaExtraParams> {
  public BaseJavacToJarStepFactory() {}

  @Override
  public JavaExtraParams castExtraParams(ExtraParams extraParams) {
    return (JavaExtraParams) extraParams;
  }

  @Override
  public void createCompileStep(
      RelPath bsmrOut,
      AbsPath buildCellRootPath,
      BuildTargetValue invokingRule,
      CompilerOutputPathsValue compilerOutputPathsValue,
      CompilerParameters parameters,
      ImmutableList.Builder<IsolatedStep> steps,
      ResolvedJavac resolvedJavac,
      @Nullable ActionMetadata actionMetadata,
      JavaExtraParams extraParams,
      JarParameters abiJarParameters,
      boolean mixedCompilation) {

    CompilerOutputPaths outputPath = compilerOutputPathsValue.getByType(invokingRule.getType());
    if (extraParams.getAddAnnotationPath()) {
      addAnnotationGenFolderStep(steps, outputPath.getAnnotationPath());
    }

    ResolvedJavacOptions resolvedJavacOptions = extraParams.getResolvedJavacOptions();
    steps.add(
        new JavacStep(
            resolvedJavac,
            resolvedJavacOptions,
            invokingRule,
            bsmrOut,
            compilerOutputPathsValue,
            parameters,
            abiJarParameters,
            null,
            mixedCompilation));
  }

  protected void addAnnotationGenFolderStep(
      ImmutableList.Builder<IsolatedStep> steps, RelPath annotationGenFolder) {
    steps.add(new MkdirIsolatedStep(annotationGenFolder));
  }
}
