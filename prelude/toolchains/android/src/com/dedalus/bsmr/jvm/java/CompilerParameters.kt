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

import com.dedalus.bsmr.cd.model.java.AbiGenerationMode
import com.dedalus.bsmr.core.filesystems.RelPath
import com.dedalus.bsmr.jvm.java.abi.source.api.SourceOnlyAbiRuleInfoFactory
import com.google.common.collect.ImmutableList
import com.google.common.collect.ImmutableSortedSet

data class CompilerParameters(
    val sourceFilePaths: ImmutableSortedSet<RelPath>,
    val classpathEntries: ImmutableList<RelPath>,
    val classpathSnapshots: ImmutableList<RelPath>,
    val outputPaths: CompilerOutputPaths,
    val abiGenerationMode: AbiGenerationMode,
    val abiCompatibilityMode: AbiGenerationMode,
    val shouldTrackClassUsage: Boolean,
    val sourceOnlyAbiRuleInfoFactory: SourceOnlyAbiRuleInfoFactory?,
)
