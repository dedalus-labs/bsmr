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

package com.dedalus.bsmr.jvm.java;

import com.dedalus.bsmr.core.filesystems.AbsPath;
import com.dedalus.bsmr.core.filesystems.RelPath;
import com.dedalus.bsmr.io.filesystem.impl.ProjectFilesystemUtils;
import com.dedalus.bsmr.util.json.ObjectMappers;
import com.google.common.base.Preconditions;
import com.google.common.collect.ImmutableMap;
import com.google.common.collect.ImmutableSortedMap;
import com.google.common.collect.ImmutableSortedSet;
import java.io.IOException;
import java.nio.file.Path;
import java.util.Map;
import java.util.Set;

public class DefaultClassUsageFileWriter implements ClassUsageFileWriter {

  @Override
  public void writeFile(
      ImmutableMap<Path, Set<Path>> classUsages,
      RelPath relativePath,
      AbsPath rootPath,
      RelPath configuredOutput) {
    try {
      AbsPath parent = rootPath.resolve(relativePath).getParent();
      Path parentRelativePath = rootPath.relativize(parent).getPath();
      Preconditions.checkState(
          ProjectFilesystemUtils.exists(rootPath, parentRelativePath),
          "directory must exist: %s",
          parent);

      ObjectMappers.WRITER.writeValue(
          rootPath.resolve(relativePath).toFile(),
          relativizeMap(classUsages, rootPath, configuredOutput));
    } catch (IOException e) {
      throw new RuntimeException("Unable to write used classes file.", e);
    }
  }

  protected static ImmutableSortedMap<Path, Set<Path>> relativizeMap(
      ImmutableMap<Path, Set<Path>> classUsages, AbsPath rootPath, RelPath configuredOutput) {
    ImmutableSortedMap.Builder<Path, Set<Path>> builder = ImmutableSortedMap.naturalOrder();

    for (Map.Entry<Path, Set<Path>> jarClassesEntry : classUsages.entrySet()) {
      Path jarAbsolutePath = jarClassesEntry.getKey();
      // Don't include jars that are outside of the project
      // Paths outside the project would make these class usage files problematic for caching.
      // Fortunately, such paths are also not interesting for the main use cases that these files
      // address, namely understanding the dependencies one java rule has on others. Jar files
      // outside the project are coming from a build tool (e.g. JDK or Android SDK).
      // Here we first check to see if the path is inside the root filesystem of the build
      // If not, we check to see if it's under one of the other cell roots.
      // If the the absolute path does not reside under any cell root, we exclude it
      ProjectFilesystemUtils.getPathRelativeToProjectRoot(
              rootPath, configuredOutput, jarAbsolutePath)
          .ifPresent(
              projectPath ->
                  builder.put(projectPath, ImmutableSortedSet.copyOf(jarClassesEntry.getValue())));
    }

    return builder.build();
  }
}
