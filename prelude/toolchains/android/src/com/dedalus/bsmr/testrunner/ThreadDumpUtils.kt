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

package com.dedalus.bsmr.testrunner

/** Utility for capturing JVM thread dumps for timeout diagnostics. */
object ThreadDumpUtils {

  /** Captures a thread dump of all live threads and returns it as a string. */
  @JvmStatic
  fun capture(): String {
    val sb = StringBuilder()
    Thread.getAllStackTraces().forEach { (thread, stackTrace) ->
      sb.appendLine("Thread: ${thread.name} (${thread.state})")
      stackTrace.forEach { element -> sb.appendLine("  at $element") }
      sb.appendLine()
    }
    return sb.toString()
  }

  /** Prints a thread dump to stderr for log visibility. */
  @JvmStatic
  fun print() {
    System.err.println("\n=== Thread Dump ===")
    System.err.println(capture())
  }
}
