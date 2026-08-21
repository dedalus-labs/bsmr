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

import org.jetbrains.kotlin.buildtools.api.KotlinLogger // @oss-enable
import com.dedalus.bsmr.core.util.log.Logger
import com.dedalus.bsmr.jvm.kotlin.cd.analytics.KotlinCDLoggingContext
// @oss-disable: import com.facebook.kotlin.compilercompat.KotlinLoggerCompat
import java.io.PrintStream
import java.util.logging.Level

internal class BsmrKotlinLogger(
    private val stdErr: PrintStream,
    private val loggingContext: KotlinCDLoggingContext,
)
: KotlinLogger // @oss-enable
// @oss-disable: : KotlinLoggerCompat()
{

  override val isDebugEnabled: Boolean
    get() = LOG.isDebugEnabled

  override fun debug(msg: String) {
    if (!LOG.isDebugEnabled) return
    LOG.debug(msg)
  }

  override fun error(msg: String, throwable: Throwable?) {
    if (!LOG.isLoggable(Level.SEVERE)) return
    stdErr.println(msg)
    throwable?.printStackTrace(stdErr)
  }

  override fun info(msg: String) {
    if (msg.startsWith("Non-incremental compilation will be performed")) {
      loggingContext.addExtras(BsmrKotlinLogger::class.java.simpleName, msg)
    }
    if (msg.startsWith("KOTLIN_BUILD_METRIC|")) {
      loggingContext.addExtras("BuildMetrics", msg.removePrefix("KOTLIN_BUILD_METRIC|"))
    }

    if (!LOG.isLoggable(Level.INFO)) return
    LOG.info(msg)
  }

  override fun lifecycle(msg: String) {
    if (!LOG.isLoggable(Level.INFO)) return
    LOG.info(msg)
  }

  override fun warn(msg: String) {
    if (!LOG.isLoggable(Level.WARNING)) return
    stdErr.println(msg)
  }

  companion object {
    private val LOG: Logger = Logger.get(BsmrKotlinLogger::class.java)
  }
}
