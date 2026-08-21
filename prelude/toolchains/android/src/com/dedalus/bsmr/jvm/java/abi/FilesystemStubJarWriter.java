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

package com.dedalus.bsmr.jvm.java.abi;

import com.dedalus.bsmr.core.filesystems.AbsPath;
import com.dedalus.bsmr.io.pathformat.PathFormatter;
import com.dedalus.bsmr.util.function.ThrowingSupplier;
import com.dedalus.bsmr.util.zip.CustomZipEntry;
import com.dedalus.bsmr.util.zip.JarBuilder;
import com.dedalus.bsmr.util.zip.JarEntrySupplier;
import java.io.IOException;
import java.io.InputStream;
import java.nio.file.Path;

/** A {@link StubJarWriter} that writes to a file. */
class FilesystemStubJarWriter implements StubJarWriter {

  private final AbsPath outputPath;
  private final JarBuilder jarBuilder;
  private boolean closed = false;

  public FilesystemStubJarWriter(AbsPath outputPath) {
    this.outputPath = outputPath;
    this.jarBuilder = new JarBuilder().setShouldHashEntries(true).setShouldMergeManifests(true);
  }

  @Override
  public void writeEntry(
      Path relativePath, ThrowingSupplier<InputStream, IOException> streamSupplier) {
    jarBuilder.addEntry(
        new JarEntrySupplier(
            new CustomZipEntry(PathFormatter.pathWithUnixSeparators(relativePath)),
            streamSupplier));
  }

  @Override
  public void close() throws IOException {
    if (!closed) {
      jarBuilder.createJarFile(outputPath.getPath());
    }
    closed = true;
  }
}
