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

package com.dedalus.bsmr.jvm.cd.serialization;

import com.dedalus.bsmr.core.filesystems.AbsPath;
import com.dedalus.bsmr.core.filesystems.RelPath;
import com.dedalus.bsmr.util.environment.EnvVariablesProvider;
import com.facebook.infer.annotation.Nullsafe;
import java.nio.file.Path;
import java.nio.file.Paths;

/** {@link AbsPath} to protobuf serializer */
@Nullsafe(Nullsafe.Mode.LOCAL)
public class AbsPathSerializer {
  private static final String EXPECT_RELATIVE_PATHS_ENV_VAR =
      "JAVACD_ABSOLUTE_PATHS_ARE_RELATIVE_TO_CWD";

  private static final boolean EXPECT_RELATIVE_PATHS =
      EnvVariablesProvider.getSystemEnv().get(EXPECT_RELATIVE_PATHS_ENV_VAR) != null;

  private AbsPathSerializer() {}

  /** Deserializes javacd model into {@link AbsPath}. */
  public static AbsPath deserialize(String absPath) {
    Path path = Paths.get(absPath);
    if (EXPECT_RELATIVE_PATHS) {
      RelPath relPath = RelPath.of(path);

      return AbsPath.of(Paths.get("").toAbsolutePath()).resolve(relPath);
    } else {
      return AbsPath.of(path);
    }
  }
}
