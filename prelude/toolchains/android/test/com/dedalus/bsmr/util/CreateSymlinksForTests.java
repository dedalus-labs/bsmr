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

package com.dedalus.bsmr.util;

import com.dedalus.bsmr.core.filesystems.AbsPath;
import com.dedalus.bsmr.core.filesystems.PathWrapper;
import com.dedalus.bsmr.io.file.MorePaths;
import com.dedalus.bsmr.io.windowsfs.WindowsFS;
import java.io.IOException;
import java.nio.file.Path;

public class CreateSymlinksForTests {
  private static final WindowsFS winFS;

  static {
    winFS = new WindowsFS();
  }

  /**
   * Creates a symlink using platform specific implementations, if there are some.
   *
   * @param symLink symlink to create.
   * @param realFile target of the symlink.
   * @throws IOException
   */
  public static void createSymLink(Path symLink, Path realFile) throws IOException {
    MorePaths.createSymLink(winFS, symLink, realFile);
  }

  /**
   * Creates a symlink using platform specific implementations, if there are some.
   *
   * @param symLink symlink to create.
   * @param realFile target of the symlink.
   * @throws IOException
   */
  public static void createSymLink(AbsPath symLink, PathWrapper realFile) throws IOException {
    createSymLink(symLink.getPath(), realFile.getPath());
  }
}
