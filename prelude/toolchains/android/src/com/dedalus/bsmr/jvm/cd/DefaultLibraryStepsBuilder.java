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

package com.dedalus.bsmr.jvm.cd;

import com.dedalus.bsmr.cd.model.java.AbiGenerationMode;
import com.dedalus.bsmr.core.filesystems.AbsPath;
import com.dedalus.bsmr.core.filesystems.RelPath;
import com.dedalus.bsmr.jvm.core.BuildTargetValue;
import com.dedalus.bsmr.jvm.java.ActionMetadata;
import com.dedalus.bsmr.jvm.java.CompileToJarStepFactory;
import com.dedalus.bsmr.jvm.java.CompilerOutputPathsValue;
import com.dedalus.bsmr.jvm.java.CompilerParameters;
import com.dedalus.bsmr.jvm.java.JarParameters;
import com.dedalus.bsmr.jvm.java.ResolvedJavac;
import com.dedalus.bsmr.step.isolatedsteps.common.MakeCleanDirectoryIsolatedStep;
import com.dedalus.bsmr.step.isolatedsteps.java.MakeMissingOutputsStep;
import com.facebook.infer.annotation.Nullsafe;
import com.google.common.collect.ImmutableList;
import com.google.common.collect.ImmutableMap;
import com.google.common.collect.ImmutableSortedSet;
import org.jetbrains.annotations.Nullable;

/** Default implementation of {@link LibraryStepsBuilder} */
@Nullsafe(Nullsafe.Mode.LOCAL)
class DefaultLibraryStepsBuilder<T extends CompileToJarStepFactory.ExtraParams>
    extends DefaultCompileStepsBuilderBase<T> implements LibraryStepsBuilder {

  DefaultLibraryStepsBuilder(CompileToJarStepFactory<T> configuredCompiler) {
    super(configuredCompiler);
  }

  @Override
  public void addMakeMissingOutputsStep(RelPath annotationsPath) {
    stepsBuilder.add(new MakeMissingOutputsStep(annotationsPath));
  }

  @Override
  public void addBuildStepsForLibrary(
      AbiGenerationMode abiCompatibilityMode,
      AbiGenerationMode abiGenerationMode,
      boolean isRequiredForSourceOnlyAbi,
      boolean trackClassUsage,
      RelPath bsmrOut,
      BuildTargetValue buildTargetValue,
      CompilerOutputPathsValue compilerOutputPathsValue,
      ImmutableList<RelPath> compileTimeClasspathPaths,
      ImmutableList<RelPath> compileTimeClasspathSnapshotPaths,
      ImmutableSortedSet<RelPath> javaSrcs,
      ImmutableMap<RelPath, RelPath> resourcesMap,
      @Nullable JarParameters libraryJarParameters,
      AbsPath buildCellRootPath,
      ResolvedJavac resolvedJavac,
      @Nullable ActionMetadata actionMetadata,
      CompileToJarStepFactory.ExtraParams extraParams) {

    CompilerParameters compilerParameters =
        JavaLibraryRules.getCompilerParameters(
            compileTimeClasspathPaths,
            compileTimeClasspathSnapshotPaths,
            javaSrcs,
            buildTargetValue.getFullyQualifiedName(),
            trackClassUsage,
            abiGenerationMode,
            abiCompatibilityMode,
            isRequiredForSourceOnlyAbi,
            compilerOutputPathsValue.getByType(buildTargetValue.getType()));

    stepsBuilder.addAll(
        MakeCleanDirectoryIsolatedStep.of(
            compilerOutputPathsValue.getByType(buildTargetValue.getType()).getWorkingDirectory()));

    configuredCompiler.createCompileToJarStep(
        bsmrOut,
        buildCellRootPath,
        buildTargetValue,
        compilerOutputPathsValue,
        compilerParameters,
        null,
        libraryJarParameters,
        stepsBuilder,
        resourcesMap,
        resolvedJavac,
        actionMetadata,
        configuredCompiler.castExtraParams(extraParams));
  }
}
