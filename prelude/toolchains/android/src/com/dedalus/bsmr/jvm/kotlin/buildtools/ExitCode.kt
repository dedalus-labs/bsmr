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

import com.dedalus.bsmr.jvm.kotlin.kotlinc.Kotlinc
import org.jetbrains.kotlin.buildtools.api.CompilationResult

/**
 * Copy of [org.jetbrains.kotlin.cli.common.ExitCode]. It allows [BuildToolsKotlinc] to implement
 * the [Kotlinc] interface without depending on the Kotlin compiler directly.
 */
internal enum class ExitCode(val code: Int) {
  OK(0),
  COMPILATION_ERROR(1),
  INTERNAL_ERROR(2),
  OOM_ERROR(137),
}

internal val CompilationResult.toExitCode: ExitCode
  get() =
      when (this) {
        CompilationResult.COMPILATION_SUCCESS -> ExitCode.OK
        CompilationResult.COMPILATION_ERROR -> ExitCode.COMPILATION_ERROR
        CompilationResult.COMPILER_INTERNAL_ERROR -> ExitCode.INTERNAL_ERROR
        CompilationResult.COMPILATION_OOM_ERROR -> ExitCode.OOM_ERROR
        else -> error("Unexpected exit code: $this")
      }
