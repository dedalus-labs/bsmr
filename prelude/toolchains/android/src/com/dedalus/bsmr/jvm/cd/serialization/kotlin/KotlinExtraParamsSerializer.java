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

package com.dedalus.bsmr.jvm.cd.serialization.kotlin;

import com.dedalus.bsmr.core.filesystems.AbsPath;
import com.dedalus.bsmr.jvm.cd.command.kotlin.KotlinExtraParams;
import com.dedalus.bsmr.jvm.cd.serialization.AbsPathSerializer;
import com.dedalus.bsmr.jvm.cd.serialization.java.ResolvedJavacOptionsSerializer;
import com.facebook.infer.annotation.Nullsafe;
import com.google.common.collect.ImmutableList;
import com.google.common.collect.ImmutableMap;
import com.google.common.collect.ImmutableSortedSet;
import java.util.Map;
import java.util.Optional;

/**
 * Marshalling between:
 *
 * <ul>
 *   <li>{@link com.dedalus.bsmr.jvm.cd.command.kotlin.KotlinExtraParams} (the parameters needed to
 *       create the steps for compiling Kotlin libraries), and
 *   <li>{@link com.dedalus.bsmr.cd.model.kotlin.KotlinExtraParams} (part of the protocol buffer
 *       model).
 * </ul>
 */
@Nullsafe(Nullsafe.Mode.LOCAL)
public class KotlinExtraParamsSerializer {

  private KotlinExtraParamsSerializer() {}

  /** Protocol buffer model to internal bsmr representation. */
  public static KotlinExtraParams deserialize(
      com.dedalus.bsmr.cd.model.java.ResolvedJavacOptions resolvedJavacOptions,
      com.dedalus.bsmr.cd.model.kotlin.KotlinExtraParams kotlinExtraParams) {
    return new KotlinExtraParams(
        kotlinExtraParams.getExtraClassPathsList().stream()
            .map(AbsPathSerializer::deserialize)
            .collect(ImmutableList.toImmutableList()),
        kotlinExtraParams.getExtraClassPathSnapshotsList().stream()
            .map(AbsPathSerializer::deserialize)
            .collect(ImmutableList.toImmutableList()),
        AbsPathSerializer.deserialize(kotlinExtraParams.getStandardLibraryClassPath()),
        AbsPathSerializer.deserialize(kotlinExtraParams.getAnnotationProcessingClassPath()),
        AnnotationProcessingToolSerializer.deserialize(
            kotlinExtraParams.getAnnotationProcessingTool()),
        kotlinExtraParams.getExtraKotlincArgumentsList().stream()
            .collect(ImmutableList.toImmutableList()),
        kotlinExtraParams.getKotlinCompilerPluginsMap().entrySet().stream()
            .collect(
                ImmutableMap.toImmutableMap(
                    e -> AbsPathSerializer.deserialize(e.getKey()),
                    e -> ImmutableMap.copyOf(e.getValue().getParamsMap()))),
        kotlinExtraParams.getKosabiPluginOptionsMap().entrySet().stream()
            .collect(
                ImmutableMap.toImmutableMap(
                    Map.Entry::getKey, e -> AbsPathSerializer.deserialize(e.getValue()))),
        Optional.ofNullable(kotlinExtraParams.getKosabiJvmAbiGenEarlyTerminationMessagePrefix()),
        kotlinExtraParams.getFriendPathsList().stream()
            .map(AbsPathSerializer::deserialize)
            .collect(ImmutableSortedSet.toImmutableSortedSet(AbsPath.comparator())),
        kotlinExtraParams.getKotlinHomeLibrariesList().stream()
            .map(AbsPathSerializer::deserialize)
            .collect(ImmutableSortedSet.toImmutableSortedSet(AbsPath.comparator()))
            .stream()
            .collect(ImmutableList.toImmutableList()),
        ResolvedJavacOptionsSerializer.deserialize(resolvedJavacOptions),
        Optional.ofNullable(kotlinExtraParams.getJvmTarget()),
        kotlinExtraParams.getShouldUseJvmAbiGen(),
        Optional.of(kotlinExtraParams.getJvmAbiGenPlugin())
            .filter(s -> !s.isEmpty())
            .map(AbsPathSerializer::deserialize),
        kotlinExtraParams.getShouldVerifySourceOnlyAbiConstraints(),
        Optional.of(kotlinExtraParams.getDepTrackerPlugin())
            .filter(s -> !s.isEmpty())
            .map(AbsPathSerializer::deserialize),
        kotlinExtraParams.getShouldKotlincRunIncrementally(),
        Optional.of(kotlinExtraParams.getIncrementalStateDir())
            .filter(s -> !s.isEmpty())
            .map(AbsPathSerializer::deserialize),
        kotlinExtraParams.getShouldKsp2RunIncrementally(),
        kotlinExtraParams.getLanguageVersion(),
        kotlinExtraParams.getShouldKosabiJvmAbiGenUseK2(),
        AbsPathSerializer.deserialize(kotlinExtraParams.getKotlinClassesDir()),
        Optional.of(kotlinExtraParams.getJavaBinary()).filter(s -> !s.isEmpty()),
        kotlinExtraParams.getApplicabilityClasspathList().stream()
            .map(AbsPathSerializer::deserialize)
            .collect(ImmutableList.toImmutableList()));
  }
}
