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

package com.dedalus.bsmr.jvm.cd.serialization.java;

import com.dedalus.bsmr.cd.model.java.ResolvedJavacOptions;
import com.dedalus.bsmr.core.filesystems.RelPath;
import com.dedalus.bsmr.jvm.cd.serialization.RelPathSerializer;
import com.dedalus.bsmr.jvm.java.ResolvedJavacPluginProperties;
import com.facebook.infer.annotation.Nullsafe;
import com.google.common.collect.ImmutableList;
import com.google.common.collect.ImmutableMap;
import com.google.common.collect.ImmutableSortedSet;
import java.util.Map;

/** {@link ResolvedJavacPluginProperties} to protobuf serializer */
@Nullsafe(Nullsafe.Mode.LOCAL)
class ResolvedJavacPluginPropertiesSerializer {

  private ResolvedJavacPluginPropertiesSerializer() {}

  /**
   * Deserializes javacd model's {@link ResolvedJavacOptions.ResolvedJavacPluginProperties} into
   * {@link ResolvedJavacPluginProperties}.
   */
  public static ResolvedJavacPluginProperties deserialize(
      ResolvedJavacOptions.ResolvedJavacPluginProperties pluginProperties) {
    return new ResolvedJavacPluginProperties(
        pluginProperties.getCanReuseClassLoader(),
        pluginProperties.getDoesNotAffectAbi(),
        pluginProperties.getSupportsAbiGenerationFromSource(),
        pluginProperties.getRunsOnJavaOnly(),
        ImmutableSortedSet.copyOf(pluginProperties.getProcessorNamesList()),
        pluginProperties.getClasspathList().stream()
            .map(RelPathSerializer::deserialize)
            .collect(ImmutableList.toImmutableList()),
        toPathParams(pluginProperties.getPathParamsMap()),
        ImmutableList.copyOf(pluginProperties.getArgumentsList()));
  }

  private static ImmutableMap<String, RelPath> toPathParams(Map<String, String> pathParamsMap) {
    ImmutableMap.Builder<String, RelPath> pathParamsBuilder =
        ImmutableMap.builderWithExpectedSize(pathParamsMap.size());
    for (Map.Entry<String, String> entry : pathParamsMap.entrySet()) {
      pathParamsBuilder.put(entry.getKey(), RelPathSerializer.deserialize(entry.getValue()));
    }
    return pathParamsBuilder.build();
  }
}
