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

class JarsActionMetadata(actionMetaData: ActionMetadata) {

  val previousJarsDigest: Map<Path, String> =
      actionMetaData.previousDigest.filter { (path, _) ->
        val relPath = RelPath.of(path)
        JAR_PATH_MATCHER.matches(relPath)
      }

  val currentJarsDigest: Map<Path, String> =
      actionMetaData.currentDigest.filter { (path, _) ->
        val relPath = RelPath.of(path)
        JAR_PATH_MATCHER.matches(relPath)
      }

  fun hasChanged(relPath: RelPath): Boolean {
    require(JAR_PATH_MATCHER.matches(relPath))

    return previousJarsDigest[relPath.path] != currentJarsDigest[relPath.path]
  }

  companion object {
    private val JAR_PATH_MATCHER = FileExtensionMatcher.of("jar")
  }
}
