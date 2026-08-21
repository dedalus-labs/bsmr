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

package com.dedalus.bsmr.zip;

import static com.dedalus.bsmr.util.zip.AppendableCentralDirectory.readCentralDirectory;
import static com.dedalus.bsmr.util.zip.ZipOutputStreams.appendJarOutputStream;
import static java.util.Arrays.asList;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertTrue;

import com.dedalus.bsmr.testutil.TemporaryPaths;
import com.dedalus.bsmr.util.zip.AppendableCentralDirectory;
import com.dedalus.bsmr.util.zip.CustomJarOutputStream;
import java.io.IOException;
import java.nio.file.Path;
import java.util.List;
import org.junit.Rule;
import org.junit.Test;

public class IncrementalJarTest extends JarTestSupport {

  @Rule public TemporaryPaths tmp = new TemporaryPaths();

  @Test
  public void testIncrementalCentralDirectory() throws IOException {

    final Path outputJar = newJar().entries(ENTRY_1, ENTRY_2, ENTRY_3).writeTo(tmp);
    final AppendableCentralDirectory directory = readCentralDirectory(outputJar);
    final List<String> expectedEntries = asList(ENTRY_1, ENTRY_2, ENTRY_3, MANIFEST);
    final List<String> valuesToCheck = asList(ENTRY_1, ENTRY_2, ENTRY_3);

    assertTrue(directory.getEntryOffset() > 0);
    assertEquals(expectedEntries.size(), directory.getEntryCount()); // manifest is added
    assertEquals(expectedEntries, readJarEntryNames(outputJar));
    assertEquals(valuesToCheck, readJarEntryValues(outputJar, valuesToCheck));
  }

  @Test
  public void testIncrementalJar() throws IOException {

    final Path outputJar = tmp.newFile().getPath();
    final Path inputJar = newJar().entries(ENTRY_1, ENTRY_2, ENTRY_3).writeTo(tmp);
    final List<String> expectedEntries =
        asList(ENTRY_1, ENTRY_2, ENTRY_3, MANIFEST, ENTRY_4, ENTRY_5, MANIFEST);
    final List<String> valuesToCheck = asList(ENTRY_1, ENTRY_2, ENTRY_3, ENTRY_4, ENTRY_5);

    try (final CustomJarOutputStream jar = appendJarOutputStream(inputJar, outputJar)) {
      newJar().entries(ENTRY_4, ENTRY_5).writeTo(jar);
    }

    final AppendableCentralDirectory directory = readCentralDirectory(outputJar);
    assertTrue(directory.getEntryOffset() > 0);
    assertEquals(expectedEntries.size(), directory.getEntryCount());
    assertEquals(expectedEntries, readJarEntryNames(outputJar));
    assertEquals(valuesToCheck, readJarEntryValues(outputJar, valuesToCheck));
  }
}
