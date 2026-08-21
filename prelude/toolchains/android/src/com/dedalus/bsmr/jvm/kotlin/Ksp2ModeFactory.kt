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

@file:JvmName("Ksp2ModeFactory")
@file:OptIn(ExperimentalPathApi::class)

package com.dedalus.bsmr.jvm.kotlin

import com.dedalus.bsmr.core.filesystems.AbsPath
import com.dedalus.bsmr.core.filesystems.RelPath
import com.dedalus.bsmr.core.util.log.Logger
import com.dedalus.bsmr.io.file.MostFiles
import com.dedalus.bsmr.jvm.cd.command.kotlin.KotlinExtraParams
import com.dedalus.bsmr.jvm.java.ActionMetadata
import com.dedalus.bsmr.jvm.kotlin.ksp.incremental.Ksp2Mode
import com.dedalus.bsmr.jvm.kotlin.ksp.incremental.ReprocessReason
import java.nio.file.Files
import kotlin.io.path.ExperimentalPathApi

@JvmName("create")
fun Ksp2Mode(
    rootProjectDir: AbsPath,
    isSourceOnly: Boolean,
    kspCachesOutput: RelPath,
    extraParams: KotlinExtraParams,
    actionMetadata: ActionMetadata?,
): Ksp2Mode {
  when {
    !extraParams.shouldKsp2RunIncrementally -> {
      LOG.info("Non-incremental mode applied: incremental property disabled")
      return Ksp2Mode.NonIncremental(kspCachesOutput)
    }
    isSourceOnly -> {
      LOG.info("Non-incremental mode applied: source-only build requested")
      return Ksp2Mode.NonIncremental(kspCachesOutput)
    }
    else -> {
      val cachesDir =
          extraParams.ksp2CachesDir.orElseThrow {
            IllegalStateException("incremental_state_dir/ksp2_caches_dir is not created")
          }
      val sourceFilesMetadata = SourceFilesActionMetadata(requireNotNull(actionMetadata))
      val snapshotsActionMetadata = SnapshotsActionMetadata(requireNotNull(actionMetadata))
      val hasClasspathChanged = snapshotsActionMetadata.hasClasspathChanged()

      if (hasClasspathChanged) {
        // TODO(ijurcikova) implement support for classpath changes and move the logic into Ksp2Step
        LOG.info(
            "Non-incremental processing will be performed: Incremental processing after classpath change is not supported yet"
        )
        createCleanDirectory(cachesDir)
      }

      LOG.info("Incremental mode applied")

      return Ksp2Mode.Incremental(
          cachesDir,
          true,
          sourceFilesMetadata.calculateAddedAndModifiedSourceFiles().map(rootProjectDir::resolve),
          sourceFilesMetadata.calculateRemovedFiles().map(rootProjectDir::resolve),
          emptyList(),
          if (hasClasspathChanged) ReprocessReason.CLASSPATH_CHANGED else null,
      )
    }
  }
}

private fun createCleanDirectory(dir: AbsPath) {
  MostFiles.deleteRecursivelyIfExists(dir.getPath())
  Files.createDirectories(dir.getPath())
}

private val LOG = Logger.get("Ksp2ModeFactory")
