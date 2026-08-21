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

package com.dedalus.bsmr.jvm.java

import com.dedalus.bsmr.jvm.java.version.utils.JavaVersionUtils

data class JavacLanguageLevelOptions(val sourceLevel: String, val targetLevel: String) {
  val sourceLevelValue: Int
    get() = JavaVersionUtils.getMajorVersionFromString(sourceLevel)

  val targetLevelValue: Int
    get() = JavaVersionUtils.getMajorVersionFromString(targetLevel)

  companion object {
    // Default combined source and target level.
    const val TARGETED_JAVA_VERSION: String = "8"

    @JvmField
    val DEFAULT: JavacLanguageLevelOptions =
        JavacLanguageLevelOptions(TARGETED_JAVA_VERSION, TARGETED_JAVA_VERSION)
  }
}
