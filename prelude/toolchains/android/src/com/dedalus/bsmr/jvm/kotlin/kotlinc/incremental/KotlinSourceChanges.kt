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

package com.dedalus.bsmr.jvm.kotlin.kotlinc.incremental

import com.dedalus.bsmr.core.filesystems.AbsPath

/** A hierarchy representing source files changes for incremental compilation */
sealed interface KotlinSourceChanges {
  /**
   * A marker object stating that the API consumer is not capable of calculating source file
   * changes.
   */
  data object ToBeCalculated : KotlinSourceChanges

  /**
   * A class containing [modifiedFiles] and [removedFiles] calculated from source file changes by
   * the API consumer.
   */
  data class Known(val addedAndModifiedFiles: List<AbsPath>, val removedFiles: List<AbsPath>) :
      KotlinSourceChanges
}
