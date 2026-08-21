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

import com.dedalus.bsmr.core.filesystems.RelPath;
import com.dedalus.bsmr.jvm.cd.serialization.RelPathSerializer;
import com.dedalus.bsmr.jvm.java.ResolvedJavacOptions;
import com.facebook.infer.annotation.Nullsafe;
import com.google.common.collect.ImmutableList;
import java.util.Optional;

/** {@link ResolvedJavacOptions} to protobuf serializer */
@Nullsafe(Nullsafe.Mode.LOCAL)
public class ResolvedJavacOptionsSerializer {

  private ResolvedJavacOptionsSerializer() {}

  /**
   * Deserializes javacd model's {@link com.dedalus.bsmr.cd.model.java.ResolvedJavacOptions} into
   * {@link ResolvedJavacOptions}.
   */
  public static ResolvedJavacOptions deserialize(
      com.dedalus.bsmr.cd.model.java.ResolvedJavacOptions options) {
    var bootclasspathListList = options.getBootclasspathListList();
    ImmutableList<RelPath> bootclasspathList =
        bootclasspathListList.stream()
            .map(RelPathSerializer::deserialize)
            .collect(ImmutableList.toImmutableList());

    return new ResolvedJavacOptions(
        bootclasspathList,
        JavacLanguageLevelOptionsSerializer.deserialize(options.getLanguageLevelOptions()),
        options.getDebug(),
        options.getVerbose(),
        JavacPluginParamsSerializer.deserialize(options.getJavaAnnotationProcessorParams()),
        JavacPluginParamsSerializer.deserialize(options.getStandardJavacPluginParams()),
        ImmutableList.copyOf(options.getExtraArgumentsList()),
        options.getSystemImage().isEmpty() ? null : options.getSystemImage());
  }

  private static Optional<String> toOptionalString(String value) {
    return value.isEmpty() ? Optional.empty() : Optional.of(value);
  }
}
