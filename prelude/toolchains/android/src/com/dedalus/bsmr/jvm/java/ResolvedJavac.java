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
import com.dedalus.bsmr.util.Escaper;
import com.google.common.collect.ImmutableList;
import com.google.common.collect.ImmutableSortedSet;
import java.util.function.Function;
import javax.annotation.Nullable;

/** Interface for a resolved javac tool. */
public interface ResolvedJavac {

  /** An escaper for arguments written to @argfiles. */
  Function<String, String> ARGFILES_ESCAPER = Escaper.javacEscaper();

  /** Prepares an invocation of the compiler with the given parameters. */
  ResolvedJavac.Invocation newBuildInvocation(
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
      @Nullable SourceOnlyAbiRuleInfoFactory ruleInfoFactory);

  String getDescription(ImmutableList<String> options, RelPath pathToSrcsList);

  /** Returns a short name of the tool */
  String getShortName();

  /** Interface that defines invocation object created during java compilation. */
  interface Invocation extends AutoCloseable {

    /**
     * Produces a source-only ABI jar. {@link #buildClasses} may not be called on an invocation on
     * which this has been called.
     */
    int buildSourceOnlyAbiJar(boolean isMixedModule) throws InterruptedException;

    /** Produces a source ABI jar. Must be called before {@link #buildClasses} */
    int buildSourceAbiJar() throws InterruptedException;

    int buildClasses() throws InterruptedException;

    @Override
    void close();
  }
}
