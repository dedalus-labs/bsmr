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

package com.dedalus.bsmr.android.exopackage

import com.dedalus.bsmr.core.filesystems.AbsPath
import com.google.common.collect.ImmutableList
import java.util.Optional

/**
 * Isolated ExopackageInfo. Doesn't have any references to SourcePath and other bsmr's internal data
 * structures.
 */
data class IsolatedExopackageInfo(
    val dexInfo: Optional<IsolatedDexInfo>,
    val nativeLibsInfo: Optional<IsolatedNativeLibsInfo>,
    val resourcesInfo: Optional<IsolatedResourcesInfo>,
) {
  /** Isolated DexInfo */
  data class IsolatedDexInfo(val metadata: AbsPath, val directory: AbsPath)

  /** Isolated NativeLibsInfo */
  data class IsolatedNativeLibsInfo(val metadata: AbsPath, val directory: AbsPath)

  /** Isolated ResourcesInfo */
  data class IsolatedResourcesInfo(val resourcesPaths: ImmutableList<IsolatedExopackagePathAndHash>)

  /** Isolated ExopackagePathAndHash */
  data class IsolatedExopackagePathAndHash(val path: AbsPath, val hashPath: AbsPath)
}
