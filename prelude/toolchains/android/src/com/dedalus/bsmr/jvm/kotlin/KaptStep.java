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
import com.dedalus.bsmr.jvm.cd.command.kotlin.LanguageVersion;
import com.dedalus.bsmr.jvm.core.BuildTargetValue;
import com.dedalus.bsmr.jvm.java.CompilerOutputPaths;
import com.dedalus.bsmr.jvm.kotlin.cd.analytics.KotlinCDAnalytics;
import com.dedalus.bsmr.jvm.kotlin.kotlinc.Kotlinc;
import com.dedalus.bsmr.jvm.kotlin.kotlinc.incremental.KotlincMode;
import com.google.common.collect.ImmutableList;
import com.google.common.collect.ImmutableMap;
import com.google.common.collect.ImmutableSortedSet;
import java.nio.file.Path;
import java.util.Optional;

public class KaptStep extends KotlincStep {

  private static final String VERBOSE = "-verbose";

  KaptStep(
      BuildTargetValue invokingRule,
      Path outputDirectory,
      ImmutableSortedSet<RelPath> sourceFilePaths,
      Path pathToSrcsList,
      ImmutableList<AbsPath> combinedClassPathEntries,
      ImmutableList<AbsPath> kotlinHomeLibraries,
      RelPath reportDirPath,
      Kotlinc kotlinc,
      ImmutableList<String> extraArguments,
      CompilerOutputPaths outputPaths,
      RelPath configuredOutput,
      KotlinCDAnalytics kotlinCDAnalytics,
      LanguageVersion languageVersion) {
    super(
        invokingRule,
        outputDirectory,
        sourceFilePaths,
        pathToSrcsList,
        combinedClassPathEntries,
        kotlinHomeLibraries,
        reportDirPath,
        kotlinc,
        extraArguments,
        ImmutableList.of(VERBOSE),
        outputPaths,
        false,
        configuredOutput,
        ImmutableMap.of(),
        null,
        true,
        ImmutableList.of(),
        ImmutableList.of(),
        false,
        ImmutableList.of(),
        Optional.empty(),
        KotlincMode.NonIncremental.INSTANCE,
        kotlinCDAnalytics,
        languageVersion,
        // Flag turning on/off K2 support for jvm-abi-gen actions
        // not part of KAPT, since jvm-abi-gen doesn't run on KAPT steps
        false);
  }

  @Override
  public String getShortName() {
    return "kapt";
  }
}
