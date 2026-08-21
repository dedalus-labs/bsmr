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

import com.dedalus.bsmr.cd.model.java.OutputPathsValue;
import com.dedalus.bsmr.core.filesystems.RelPath;
import com.dedalus.bsmr.jvm.cd.serialization.RelPathSerializer;
import com.dedalus.bsmr.jvm.java.CompilerOutputPaths;
import com.facebook.infer.annotation.Nullsafe;
import java.util.Optional;

/** {@link CompilerOutputPaths} to protobuf serializer */
@Nullsafe(Nullsafe.Mode.LOCAL)
public class CompilerOutputPathsSerializer {

  private CompilerOutputPathsSerializer() {}

  /**
   * Deserializes javacd model's {@link OutputPathsValue.OutputPaths} into {@link
   * CompilerOutputPaths}.
   */
  public static CompilerOutputPaths deserialize(OutputPathsValue.OutputPaths outputPaths) {
    return deserialize(outputPaths, Optional.empty());
  }

  public static CompilerOutputPaths deserialize(
      OutputPathsValue.OutputPaths outputPaths, Optional<RelPath> tmpDir) {
    return new CompilerOutputPaths(
        toRelPath(outputPaths.getClassesDir()),
        toRelPath(outputPaths.getOutputJarDirPath()),
        toOptionalRelPath(outputPaths.getAbiJarPath()),
        toRelPath(outputPaths.getAnnotationPath()),
        outputPaths.getPathToSourcesList().isEmpty()
            ? tmpDir.map(p -> p.resolveRel("__srcs__")).get()
            : toRelPath(outputPaths.getPathToSourcesList()),
        outputPaths.getWorkingDirectory().isEmpty()
            ? tmpDir.get()
            : toRelPath(outputPaths.getWorkingDirectory()),
        toOptionalRelPath(outputPaths.getOutputJarPath()));
  }

  private static RelPath toRelPath(String relPath) {
    return RelPathSerializer.deserialize(relPath);
  }

  private static Optional<RelPath> toOptionalRelPath(String value) {
    return Optional.of(value)
        .filter(s -> !s.isEmpty())
        .map(CompilerOutputPathsSerializer::toRelPath);
  }
}
