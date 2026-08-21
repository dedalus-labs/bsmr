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

package com.dedalus.bsmr.jvm.kotlin.buildtools.snapshot

import com.dedalus.bsmr.core.util.log.Logger
import java.nio.file.Path
import kotlin.system.measureTimeMillis
import org.jetbrains.kotlin.buildtools.api.CompilationService
import org.jetbrains.kotlin.buildtools.api.ExperimentalBuildToolsApi

@OptIn(ExperimentalBuildToolsApi::class)
class ClasspathSnapshotGenerator(
    private val inputJar: Path,
    private val outputSnapshot: Path,
    private val granularity: SnapshotGranularity,
) {

  fun run() {
    val timeSpent = measureTimeMillis {
      val compilationService = CompilationService.loadImplementation(this.javaClass.classLoader!!)
      val snapshot =
          compilationService.calculateClasspathSnapshot(
              inputJar.toFile(),
              granularity.toClassSnapshotGranularity,
          )
      snapshot.saveSnapshot(outputSnapshot.toFile())
    }
    LOG.info("Classpath snapshot generation took $timeSpent ms for input jar: $inputJar")
  }

  companion object {
    private val LOG: Logger = Logger.get(ClasspathSnapshotGenerator::class.java)
  }
}
