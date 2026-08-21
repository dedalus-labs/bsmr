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

package com.dedalus.bsmr.util.relativepathmap;

import com.dedalus.bsmr.core.filesystems.AbsPath;
import com.dedalus.bsmr.io.filesystem.impl.ProjectFilesystemUtils;
import com.google.common.collect.ImmutableSet;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Map;

/** Utilities method related to process relative path maps */
public class RelativePathMapUtils {

  /** Adds path to relative map */
  public static void addPathToRelativePathMap(
      AbsPath rootPath,
      Map<Path, AbsPath> relativePathMap,
      Path basePath,
      AbsPath absolutePath,
      Path relativePath)
      throws IOException {
    if (Files.isDirectory(absolutePath.getPath())) {
      ImmutableSet<Path> files =
          ProjectFilesystemUtils.getFilesUnderPath(
              rootPath,
              absolutePath.getPath(),
              ProjectFilesystemUtils.getDefaultVisitOptions(),
              ProjectFilesystemUtils.getEmptyIgnoreFilter());
      for (Path file : files) {
        AbsPath absoluteFilePath = rootPath.resolve(file).normalize();
        addToRelativePathMap(
            relativePathMap, basePath.relativize(absoluteFilePath.getPath()), absoluteFilePath);
      }
    } else {
      addToRelativePathMap(relativePathMap, relativePath, absolutePath);
    }
  }

  private static void addToRelativePathMap(
      Map<Path, AbsPath> relativePathMap, Path pathRelativeToBaseDir, AbsPath absoluteFilePath) {
    relativePathMap.compute(
        pathRelativeToBaseDir,
        (ignored, current) -> {
          if (current != null) {
            throw new RuntimeException(
                String.format(
                    "The file '%s' appears twice in the hierarchy",
                    pathRelativeToBaseDir.getFileName()));
          }
          return absoluteFilePath;
        });
  }
}
