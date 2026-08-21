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
import com.google.common.collect.ImmutableList;
import com.google.common.collect.ImmutableMap;
import javax.annotation.Nullable;

/**
 * Interface for adding the steps for compiling source to producing a JAR, for JVM languages.
 *
 * @param <T> Type of extra parameters needed to create these steps.
 */
public interface CompileToJarStepFactory<T extends CompileToJarStepFactory.ExtraParams> {

  /**
   * Add the steps to {@code steps} to compile the sources (in {@code compilerParameters}) with java
   * compiler {@code resolvedJavac} for the build target {@code buildTargetValue} to a JAR, located
   * in {@code compilerOutputPathsValue}.
   *
   * <p>Language-specific parameters are passed through {@code extraParams}, which is instantiated
   * to a concrete type by implementations of this interface.
   */
  void createCompileToJarStep(
      RelPath bsmrOut,
      AbsPath buildCellRootPath,
      BuildTargetValue buildTargetValue,
      CompilerOutputPathsValue compilerOutputPathsValue,
      CompilerParameters compilerParameters,
      @Nullable JarParameters abiJarParameters,
      @Nullable JarParameters libraryJarParameters,
      ImmutableList.Builder<IsolatedStep> steps,
      ImmutableMap<RelPath, RelPath> resourcesMap,
      ResolvedJavac resolvedJavac,
      @Nullable ActionMetadata actionMetadata,
      T extraParams);

  /** Upcasts {@code extraParams} to the type of parameter expected by this factory. */
  T castExtraParams(ExtraParams extraParams);

  /** Extra params marker interface. */
  interface ExtraParams {}
}
