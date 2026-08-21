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

package com.dedalus.bsmr.jvm.kotlin;

import com.dedalus.bsmr.core.filesystems.AbsPath;
import com.dedalus.bsmr.core.util.log.Logger;
import com.dedalus.bsmr.jvm.kotlin.kotlinc.incremental.ClasspathChanges;
import com.google.common.collect.ImmutableList;
import java.util.stream.Collectors;

public class ClasspathChangesFactory {

  private static final Logger LOG = Logger.get(KotlinSourceChangesFactory.class);

  private ClasspathChangesFactory() {}

  public static ClasspathChanges create(
      SnapshotsActionMetadata actionMetadata, ImmutableList<AbsPath> classpathSnapshots) {
    if (actionMetadata.hasClasspathChanged()) {
      ImmutableList<java.io.File> snapshotFiles =
          ImmutableList.copyOf(
              classpathSnapshots.stream().map(AbsPath::toFile).collect(Collectors.toList()));

      LOG.info("Classpath changes: Detected changes on the classpath");
      return new ClasspathChanges.ToBeComputedByIncrementalCompiler(snapshotFiles);
    }

    LOG.info("Classpath changes: No changes on the classpath");
    return new ClasspathChanges.NoChanges(
        ImmutableList.copyOf(
            classpathSnapshots.stream().map(AbsPath::toFile).collect(Collectors.toList())));
  }
}
