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

package com.dedalus.bsmr.step.isolatedsteps.common

import com.dedalus.bsmr.core.build.execution.context.IsolatedExecutionContext
import com.dedalus.bsmr.core.filesystems.AbsPath
import com.dedalus.bsmr.core.filesystems.RelPath
import com.dedalus.bsmr.io.file.MostFiles
import com.dedalus.bsmr.io.filesystem.impl.ProjectFilesystemUtils
import com.dedalus.bsmr.step.StepExecutionResult
import com.dedalus.bsmr.step.StepExecutionResults
import com.dedalus.bsmr.step.isolatedsteps.IsolatedStep
import com.google.common.base.Preconditions
import com.google.common.collect.ImmutableSet
import java.io.IOException
import java.nio.file.Files
import java.util.StringJoiner

/** Removes a path if it exists. */
data class RmIsolatedStep(
    val path: RelPath,
    val isRecursive: Boolean,
    val excludedPaths: ImmutableSet<RelPath>,
) : IsolatedStep {
  override fun getShortName(): String {
    return "rm"
  }

  @Throws(IOException::class)
  override fun executeIsolatedStep(context: IsolatedExecutionContext): StepExecutionResult {
    val absolutePath = getAbsPath(context)

    if (isRecursive) {
      // Delete a folder recursively
      MostFiles.deleteRecursivelyIfExists(absolutePath, getAbsoluteExcludedPaths(context))
    } else {
      // Delete a single file
      Preconditions.checkState(
          excludedPaths.isEmpty(),
          "Excluded paths only valid for recursive steps",
      )
      Files.deleteIfExists(absolutePath.path)
    }
    return StepExecutionResults.SUCCESS
  }

  private fun getAbsPath(context: IsolatedExecutionContext): AbsPath {
    return convertRelPathToAbsPath(context, path)
  }

  private fun convertRelPathToAbsPath(
      context: IsolatedExecutionContext,
      relPath: RelPath,
  ): AbsPath {
    return ProjectFilesystemUtils.getAbsPathForRelativePath(context.ruleCellRoot, relPath)
  }

  private fun getAbsoluteExcludedPaths(context: IsolatedExecutionContext): ImmutableSet<AbsPath> {
    val excludedRelPaths = excludedPaths
    if (excludedRelPaths.isEmpty()) {
      // Avoid calls to .stream() and creation of new sets as almost all steps will not have
      // any excluded paths
      return ImmutableSet.of()
    }

    return excludedRelPaths
        .stream()
        .map { relPath: RelPath -> convertRelPathToAbsPath(context, relPath) }
        .collect(ImmutableSet.toImmutableSet())
  }

  override fun getIsolatedStepDescription(context: IsolatedExecutionContext): String {
    val args = StringJoiner(" ")
    args.add("rm")
    args.add("-f")

    if (isRecursive) {
      args.add("-r")
    }

    args.add(getAbsPath(context).toString())

    if (!excludedPaths.isEmpty()) {
      args.add("(with excluded paths)")
    }

    return args.toString()
  }
}
