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

package com.dedalus.bsmr.jvm.kotlin.kotlinc;

import com.dedalus.bsmr.core.build.execution.context.IsolatedExecutionContext;
import com.dedalus.bsmr.core.filesystems.AbsPath;
import com.dedalus.bsmr.core.filesystems.RelPath;
import com.dedalus.bsmr.jvm.core.BuildTargetValue;
import com.dedalus.bsmr.jvm.kotlin.cd.analytics.KotlinCDLoggingContext;
import com.dedalus.bsmr.jvm.kotlin.kotlinc.incremental.KotlincMode;
import com.dedalus.bsmr.jvm.kotlin.util.KotlinUnarchiverKt;
import com.google.common.collect.ImmutableList;
import com.google.common.collect.ImmutableSet;
import com.google.common.collect.ImmutableSortedSet;
import java.io.IOException;
import java.nio.file.Path;
import java.util.Optional;

/** Interface for a kotlin compiler. */
public interface Kotlinc {

  int buildWithClasspath(
      IsolatedExecutionContext context,
      BuildTargetValue invokingRule,
      ImmutableList<String> options,
      ImmutableList<AbsPath> kotlinHomeLibraries,
      ImmutableSortedSet<RelPath> kotlinSourceFilePaths,
      Path pathToSrcsList,
      Optional<Path> workingDirectory,
      AbsPath ruleCellRoot,
      KotlincMode mode,
      KotlinCDLoggingContext kotlinCDLoggingContext)
      throws InterruptedException;

  String getDescription(
      ImmutableList<String> options,
      ImmutableSortedSet<RelPath> kotlinSourceFilePaths,
      Path pathToSrcsList);

  String getShortName();

  default ImmutableList<Path> getExpandedSourcePaths(
      AbsPath ruleCellRoot,
      ImmutableSet<RelPath> kotlinSourceFilePaths,
      Optional<Path> workingDirectory)
      throws IOException {

    return KotlinUnarchiverKt.getExpandedSourcePaths(
        ruleCellRoot, kotlinSourceFilePaths, workingDirectory);
  }
}
