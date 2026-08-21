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

package com.dedalus.bsmr.jvm.kotlin;

import com.dedalus.bsmr.core.filesystems.AbsPath;
import com.dedalus.bsmr.core.filesystems.RelPath;
import com.dedalus.bsmr.io.file.FileExtensionMatcher;
import com.dedalus.bsmr.io.file.PathMatcher;
import com.dedalus.bsmr.jvm.core.BuildTargetValue;
import com.dedalus.bsmr.jvm.java.BaseJavacToJarStepFactory;
import com.dedalus.bsmr.jvm.java.CompilerOutputPathsValue;
import com.dedalus.bsmr.jvm.java.CompilerParameters;
import com.dedalus.bsmr.jvm.java.JarParameters;
import com.dedalus.bsmr.jvm.java.JavaExtraParams;
import com.dedalus.bsmr.jvm.java.ResolvedJavac;
import com.dedalus.bsmr.jvm.java.ResolvedJavacOptions;
import com.dedalus.bsmr.step.isolatedsteps.IsolatedStep;
import com.google.common.collect.ImmutableList;
import com.google.common.collect.ImmutableSortedSet;

public class JavacStepsBuilder {
  private static final PathMatcher KOTLIN_PATH_MATCHER = FileExtensionMatcher.of("kt");

  /**
   * Prepares the Java compilation step for any Java files left to compile in this rule. This also
   * compiles any Java files generated from annotation processors using KAPT and KSP.
   */
  public static void prepareJavaCompilationIfNeeded(
      BuildTargetValue invokingRule,
      AbsPath buildCellRootPath,
      ImmutableList.Builder<IsolatedStep> steps,
      RelPath bsmrOut,
      CompilerOutputPathsValue compilerOutputPathsValue,
      CompilerParameters parameters,
      ResolvedJavac resolvedJavac,
      ResolvedJavacOptions resolvedJavacOptions,
      ImmutableList<RelPath> declaredClasspathEntries,
      ImmutableList<AbsPath> extraClassPaths,
      ImmutableList<RelPath> outputDirectories,
      ImmutableSortedSet.Builder<RelPath> sourceBuilder,
      JarParameters abiJarParameter) {

    // Note that this filters out only .kt files, so this keeps both .java and .src.zip files.
    ImmutableSortedSet<RelPath> javaSourceFiles =
        sourceBuilder.build().stream()
            .filter(input -> !KOTLIN_PATH_MATCHER.matches(input))
            .collect(ImmutableSortedSet.toImmutableSortedSet(RelPath.comparator()));

    // No point running javac if there is no source file
    if (javaSourceFiles.isEmpty()) {
      return;
    }

    CompilerParameters javacParameters =
        new CompilerParameters(
            javaSourceFiles,
            buildClasspathEntries(
                buildCellRootPath, outputDirectories, extraClassPaths, declaredClasspathEntries),
            parameters.getClasspathSnapshots(),
            parameters.getOutputPaths(),
            parameters.getAbiGenerationMode(),
            parameters.getAbiCompatibilityMode(),
            parameters.getShouldTrackClassUsage(),
            parameters.getSourceOnlyAbiRuleInfoFactory());

    // Indicate no annotation processing required from this factory.  It is already handled by the
    // Kotlin factory, when it resolves javac's options.
    BaseJavacToJarStepFactory javacToJarStepFactory = new BaseJavacToJarStepFactory();

    javacToJarStepFactory.createCompileStep(
        bsmrOut,
        buildCellRootPath,
        invokingRule,
        compilerOutputPathsValue,
        javacParameters,
        steps,
        resolvedJavac,
        null,
        JavaExtraParams.of(resolvedJavacOptions, /* addAnnotationPath */ false),
        abiJarParameter,
        true);
  }

  private static ImmutableList<RelPath> buildClasspathEntries(
      AbsPath buildCellRootPath,
      ImmutableList<RelPath> outputDirectories,
      ImmutableList<AbsPath> extraClassPaths,
      ImmutableList<RelPath> declaredClasspathEntries) {
    // Build classpath with outputDirectories first (preserving order), then other entries sorted
    ImmutableList.Builder<RelPath> classpathBuilder = ImmutableList.builder();

    classpathBuilder.addAll(outputDirectories);
    classpathBuilder.addAll(
        ImmutableSortedSet.orderedBy(RelPath.comparator())
            .addAll(extraClassPaths.stream().map(buildCellRootPath::relativize).iterator())
            .addAll(declaredClasspathEntries)
            .build());

    return classpathBuilder.build();
  }
}
