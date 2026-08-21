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
import com.dedalus.bsmr.core.filesystems.RelPath;
import com.dedalus.bsmr.jvm.core.BuildTargetValue;
import com.dedalus.bsmr.jvm.java.abi.source.api.SourceOnlyAbiRuleInfoFactory;
import com.google.common.collect.ImmutableList;
import com.google.common.collect.ImmutableSortedSet;
import javax.annotation.Nullable;

/** Fake implementation of {@link ResolvedJavac} for tests. */
public class FakeJavac implements ResolvedJavac {
  private final int exitCode;
  private final String stdErr;

  public FakeJavac(int exitCode, String stdErr) {
    this.exitCode = exitCode;
    this.stdErr = stdErr;
  }

  @Override
  public ResolvedJavac.Invocation newBuildInvocation(
      JavacExecutionContext context,
      BuildTargetValue invokingRule,
      CompilerOutputPathsValue compilerOutputPathsValue,
      ImmutableList<String> options,
      JavacPluginParams annotationProcessorParams,
      JavacPluginParams pluginParams,
      ImmutableSortedSet<RelPath> javaSourceFilePaths,
      RelPath pathToSrcsList,
      RelPath workingDirectory,
      boolean trackClassUsage,
      @Nullable JarParameters abiJarParameters,
      @Nullable JarParameters libraryJarParameters,
      AbiGenerationMode abiGenerationMode,
      AbiGenerationMode abiCompatibilityMode,
      @Nullable SourceOnlyAbiRuleInfoFactory ruleInfoFactory) {
    return new ResolvedJavac.Invocation() {
      @Override
      public int buildSourceOnlyAbiJar(boolean isMixedModule) {
        throw new UnsupportedOperationException();
      }

      @Override
      public int buildSourceAbiJar() {
        throw new UnsupportedOperationException();
      }

      @Override
      public int buildClasses() {
        if (exitCode != 0) {
          context.getStdErr().print(stdErr);
        }
        return exitCode;
      }

      @Override
      public void close() {
        // Nothing to do
      }
    };
  }

  @Override
  public String getDescription(ImmutableList<String> options, RelPath pathToSrcsList) {
    return String.format("fakeJavac %s %s", options, pathToSrcsList);
  }

  @Override
  public String getShortName() {
    throw new UnsupportedOperationException();
  }
}
