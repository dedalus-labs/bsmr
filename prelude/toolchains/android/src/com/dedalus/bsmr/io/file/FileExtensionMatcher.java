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

package com.dedalus.bsmr.io.file;

import com.dedalus.bsmr.core.filesystems.RelPath;
import com.google.common.io.Files;
import java.util.Objects;

/** Matcher that matches file paths with specific extension. */
public class FileExtensionMatcher implements PathMatcher {

  private final String extension;

  private FileExtensionMatcher(String extension) {
    this.extension = extension;
  }

  @Override
  public boolean equals(Object other) {
    if (this == other) {
      return true;
    }
    if (!(other instanceof FileExtensionMatcher)) {
      return false;
    }
    FileExtensionMatcher that = (FileExtensionMatcher) other;
    return Objects.equals(extension, that.extension);
  }

  @Override
  public int hashCode() {
    return Objects.hash(extension);
  }

  @Override
  public String toString() {
    return String.format("%s extension=%s", super.toString(), extension);
  }

  @Override
  public boolean matches(RelPath path) {
    return extension.equals(Files.getFileExtension(path.toString()));
  }

  public String getExtension() {
    return extension;
  }

  /**
   * @return The matcher for paths that have {@code extension} as an extension.
   */
  public static FileExtensionMatcher of(String extension) {
    return new FileExtensionMatcher(extension);
  }
}
