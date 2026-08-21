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

package com.dedalus.bsmr.jvm.kotlin

import com.dedalus.bsmr.core.filesystems.RelPath
import com.dedalus.bsmr.io.file.FileExtensionMatcher
import com.dedalus.bsmr.jvm.java.ActionMetadata
import java.nio.file.Path
import kotlin.collections.Map
import kotlin.collections.component1
import kotlin.collections.component2

class SourceFilesActionMetadata(actionMetaData: ActionMetadata) {

  val previousSourceFilesDigest: Map<Path, String> =
      actionMetaData.previousDigest.filter { (path, _) ->
        val relPath = RelPath.of(path)
        KT_PATH_MATCHER.matches(relPath) || JAVA_PATH_MATCHER.matches(relPath)
      }

  val currentSourceFilesDigest: Map<Path, String> =
      actionMetaData.currentDigest.filter { (path, _) ->
        val relPath = RelPath.of(path)
        KT_PATH_MATCHER.matches(relPath) || JAVA_PATH_MATCHER.matches(relPath)
      }

  fun calculateAddedAndModifiedSourceFiles(): List<Path> = buildList {
    for ((path, digest) in currentSourceFilesDigest) {
      if (previousSourceFilesDigest[path] != digest) {
        add(path)
      }
    }
  }

  fun calculateRemovedFiles(): List<Path> = buildList {
    for ((path, digest) in previousSourceFilesDigest) {
      if (!currentSourceFilesDigest.containsKey(path)) {
        add(path)
      }
    }
  }

  companion object {
    private val KT_PATH_MATCHER = FileExtensionMatcher.of("kt")
    private val JAVA_PATH_MATCHER = FileExtensionMatcher.of("java")
  }
}
