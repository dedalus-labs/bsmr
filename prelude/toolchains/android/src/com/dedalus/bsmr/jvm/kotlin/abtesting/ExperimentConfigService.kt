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

package com.dedalus.bsmr.jvm.kotlin.abtesting

import java.util.ServiceLoader

interface ExperimentConfigService {

  fun loadConfig(universeName: String): ExperimentConfig

  companion object {
    @JvmStatic
    fun loadImplementation(): ExperimentConfigService {
      val implementations = ServiceLoader.load(ExperimentConfigService::class.java)
      implementations.firstOrNull()
          ?: error(
              "The classpath contains no implementation for ${ExperimentConfigService::class.qualifiedName}"
          )
      return implementations.singleOrNull()
          ?: error(
              "The classpath contains more than one implementation for ${ExperimentConfigService::class.qualifiedName}"
          )
    }
  }
}
