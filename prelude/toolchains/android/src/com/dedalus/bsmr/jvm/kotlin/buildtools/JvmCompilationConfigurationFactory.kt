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

package com.dedalus.bsmr.jvm.kotlin.buildtools

import com.dedalus.bsmr.core.util.log.Logger
import com.dedalus.bsmr.jvm.kotlin.cd.analytics.KotlinCDLoggingContext
import com.dedalus.bsmr.jvm.kotlin.kotlinc.incremental.ClasspathChanges
import com.dedalus.bsmr.jvm.kotlin.kotlinc.incremental.KotlincMode
import com.dedalus.bsmr.jvm.kotlin.kotlinc.incremental.KotlincMode.Incremental
import com.dedalus.bsmr.jvm.kotlin.kotlinc.incremental.KotlincMode.NonIncremental
import org.jetbrains.kotlin.buildtools.api.CompilationService
import org.jetbrains.kotlin.buildtools.api.ExperimentalBuildToolsApi
import org.jetbrains.kotlin.buildtools.api.jvm.ClasspathSnapshotBasedIncrementalCompilationApproachParameters
import org.jetbrains.kotlin.buildtools.api.jvm.JvmCompilationConfiguration

@OptIn(ExperimentalBuildToolsApi::class)
internal class JvmCompilationConfigurationFactory(
    private val compilationService: CompilationService,
    private val kotlinCDLoggingContext: KotlinCDLoggingContext,
) {

  fun create(mode: KotlincMode): JvmCompilationConfiguration =
      when (mode) {
        is NonIncremental -> {
          compilationService.makeJvmCompilationConfiguration()
        }
        is Incremental -> {
          compilationService.makeJvmCompilationConfiguration().apply {
            useIncrementalCompilation(
                workingDirectory = mode.kotlicWorkingDir.toFile(),
                sourcesChanges = mode.kotlinSourceChanges.toSourcesChanges(),
                approachParameters =
                    ClasspathSnapshotBasedIncrementalCompilationApproachParameters(
                        newClasspathSnapshotFiles = mode.classpathChanges.classpathSnapshotFiles,
                        shrunkClasspathSnapshot =
                            mode.kotlicWorkingDir.resolve("shrunk-classpath-snapshot.bin").toFile(),
                    ),
                options =
                    makeClasspathSnapshotBasedIncrementalCompilationConfiguration().apply {
                      setRootProjectDir(mode.rootProjectDir.toFile())
                      setBuildDir(mode.buildDir.toFile())
                      usePreciseCompilationResultsBackup(true)
                      keepIncrementalCompilationCachesInMemory(true)

                      val rebuildReason = mode.rebuildReason
                      if (rebuildReason != null) {
                        LOG.info(
                            "Non-incremental compilation will be performed: ${rebuildReason.message}"
                        )
                        kotlinCDLoggingContext.addExtras(
                            JvmCompilationConfigurationFactory::class.java.simpleName,
                            "Non-incremental compilation will be performed: ${rebuildReason.message}",
                        )
                        forceNonIncrementalMode(true)
                      }

                      when (mode.classpathChanges) {
                        is ClasspathChanges.Unknown -> {
                          LOG.info(
                              "Non-incremental compilation will be performed: classpath changes not available"
                          )
                          kotlinCDLoggingContext.addExtras(
                              JvmCompilationConfigurationFactory::class.java.simpleName,
                              "Non-incremental compilation will be performed: classpath changes not available",
                          )
                          forceNonIncrementalMode(true)
                        }
                        is ClasspathChanges.NoChanges -> {
                          assureNoClasspathSnapshotsChanges(true)
                        }
                        is ClasspathChanges.ToBeComputedByIncrementalCompiler -> {
                          // The Kotlin incremental compiler handles classpath changes
                          // (additions, modifications, and removals).
                        }
                      }
                    },
            )
          }
        }
      }

  companion object {
    private val LOG: Logger = Logger.get(JvmCompilationConfigurationFactory::class.java)
  }
}
