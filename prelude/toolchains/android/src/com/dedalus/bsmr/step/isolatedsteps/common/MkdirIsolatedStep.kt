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
import com.dedalus.bsmr.core.filesystems.RelPath
import com.dedalus.bsmr.io.filesystem.impl.ProjectFilesystemUtils
import com.dedalus.bsmr.step.StepExecutionResult
import com.dedalus.bsmr.step.StepExecutionResults
import com.dedalus.bsmr.step.isolatedsteps.IsolatedStep
import com.dedalus.bsmr.util.Escaper
import java.io.IOException
import java.nio.file.Files

/** Command that runs equivalent command of `mkdir -p` on the specified directory. */
data class MkdirIsolatedStep(val dirPath: RelPath) : IsolatedStep {
  @Throws(IOException::class)
  override fun executeIsolatedStep(context: IsolatedExecutionContext): StepExecutionResult {
    Files.createDirectories(
        ProjectFilesystemUtils.getPathForRelativePath(context.ruleCellRoot, dirPath.path)
    )
    return StepExecutionResults.SUCCESS
  }

  override fun getIsolatedStepDescription(context: IsolatedExecutionContext): String {
    return String.format("mkdir -p %s", Escaper.escapeAsShellString(dirPath.toString()))
  }

  override fun getShortName(): String {
    return "mkdir"
  }
}
