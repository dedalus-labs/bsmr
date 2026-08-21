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

import com.dedalus.bsmr.core.filesystems.RelPath
import com.google.common.collect.ImmutableSortedSet
import java.util.Optional
import java.util.function.Predicate
import java.util.logging.Level

data class JarParameters(
    val hashEntries: Boolean,
    val mergeManifests: Boolean,
    val jarPath: RelPath,
    val removeEntryPredicate: Predicate<Any>,
    val entriesToJar: ImmutableSortedSet<RelPath>,
    val overrideEntriesToJar: ImmutableSortedSet<RelPath>,
    val mainClass: Optional<String>,
    val manifestFile: Optional<RelPath>,
    val duplicatesLogLevel: Level,
)
