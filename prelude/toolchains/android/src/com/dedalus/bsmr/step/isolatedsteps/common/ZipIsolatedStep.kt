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
import com.dedalus.bsmr.io.file.PathMatcher
import com.dedalus.bsmr.io.filesystem.impl.ProjectFilesystemUtils
import com.dedalus.bsmr.step.StepExecutionResult
import com.dedalus.bsmr.step.StepExecutionResults
import com.dedalus.bsmr.step.isolatedsteps.IsolatedStep
import com.dedalus.bsmr.util.zip.CustomZipEntryWithPath
import com.dedalus.bsmr.util.zip.Zip
import com.dedalus.bsmr.util.zip.ZipCompressionLevel
import com.dedalus.bsmr.util.zip.ZipOutputStreams
import com.dedalus.bsmr.util.zip.ZipOutputStreams.HandleDuplicates
import com.google.common.collect.ImmutableSet
import java.io.BufferedOutputStream
import java.io.IOException
import java.nio.file.Path
import java.util.TreeMap

/**
 * Creates or updates a zip archive.
 *
 * Note that paths added to the archive are always relative to the working directory.<br></br> For
 * example, if you're in `/dir` and you add `file.txt`, you get an archive containing just the file.
 * If you were in `/` and added `dir/file.txt`, you would get an archive containing the file within
 * a directory.
 */
data class ZipIsolatedStep(
    val rootPath: AbsPath,
    val pathToZipFile: Path,
    val ignoredPaths: ImmutableSet<PathMatcher>,
    val paths: ImmutableSet<Path>,
    val junkPaths: Boolean,
    val compressionLevel: ZipCompressionLevel,
    val baseDir: Path,
) : IsolatedStep {
  @Throws(IOException::class, InterruptedException::class)
  override fun executeIsolatedStep(context: IsolatedExecutionContext): StepExecutionResult {
    // Delete any stale zip file from a previous interrupted build.
    // This can happen when ctrl+c interrupts a build mid-execution.
    if (ProjectFilesystemUtils.exists(rootPath, pathToZipFile)) {
      ProjectFilesystemUtils.deleteFileAtPath(rootPath, pathToZipFile)
    }

    val entries: Map<String, CustomZipEntryWithPath> = TreeMap()

    BufferedOutputStream(ProjectFilesystemUtils.newFileOutputStream(rootPath, pathToZipFile)).use {
        baseOut ->
      ZipOutputStreams.newOutputStream(baseOut, HandleDuplicates.THROW_EXCEPTION).use { out ->
        /* TODO: Make this logic to avoid using exceptions.
         * If walking the file directory throws, then an empty jar file is still created.
         */
        Zip.walkBaseDirectoryToCreateEntries(
            rootPath,
            entries,
            baseDir,
            ignoredPaths,
            paths,
            junkPaths,
            compressionLevel,
        )
        Zip.writeEntriesToZip(rootPath, out, entries)
      }
    }
    return StepExecutionResults.SUCCESS
  }

  override fun getIsolatedStepDescription(context: IsolatedExecutionContext): String {
    val args = StringBuilder("zip ")

    // Don't add extra fields, neither do the Android tools.
    args.append("-X ")

    // recurse
    args.append("-r ")

    // compression level
    args.append("-").append(compressionLevel).append(" ")

    // junk paths
    if (junkPaths) {
      args.append("-j ")
    }

    // destination archive
    args.append(pathToZipFile).append(" ")

    // files to add to archive
    if (paths.isEmpty()) {
      // Add the contents of workingDirectory to archive.
      args.append("-i* ")
      args.append(". ")
    } else {
      // Add specified paths, relative to workingDirectory.
      for (path in paths) {
        args.append(path).append(" ")
      }
    }

    return args.toString()
  }

  override fun getShortName(): String {
    return "zip"
  }
}
