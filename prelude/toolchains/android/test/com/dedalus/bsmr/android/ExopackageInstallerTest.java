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

package com.dedalus.bsmr.android;

import static org.junit.Assert.assertEquals;

import com.dedalus.bsmr.android.exopackage.ExopackageInstaller;
import com.dedalus.bsmr.android.exopackage.NativeExoHelper;
import com.dedalus.bsmr.core.filesystems.AbsPath;
import com.dedalus.bsmr.core.filesystems.RelPath;
import com.dedalus.bsmr.io.filesystem.impl.ProjectFilesystemUtils;
import com.dedalus.bsmr.testutil.TemporaryPaths;
import com.google.common.base.Strings;
import com.google.common.collect.ImmutableList;
import com.google.common.collect.ImmutableMultimap;
import com.google.common.collect.ImmutableSet;
import java.io.IOException;
import java.nio.file.Path;
import java.nio.file.Paths;
import org.junit.Rule;
import org.junit.Test;
import org.junit.rules.ExpectedException;

@SuppressWarnings("PMD.AddEmptyString")
public class ExopackageInstallerTest {

  @Rule public ExpectedException thrown = ExpectedException.none();

  @Rule public final TemporaryPaths tmpDir = new TemporaryPaths();

  @Test
  public void testFilterLibrariesForAbi() {
    AbsPath libsDir = RelPath.get("example").toAbsolutePath();
    ImmutableMultimap<String, Path> allLibs =
        ImmutableMultimap.of(
            Strings.repeat("a", 40), libsDir.resolve("libs/armeabi-v7a/libmy1.so").getPath(),
            Strings.repeat("b", 40), libsDir.resolve("libs/armeabi-v7a/libmy2.so").getPath(),
            Strings.repeat("c", 40), libsDir.resolve("libs/armeabi/libmy2.so").getPath(),
            Strings.repeat("d", 40), libsDir.resolve("libs/armeabi/libmy3.so").getPath(),
            Strings.repeat("e", 40), libsDir.resolve("libs/x86/libmy1.so").getPath());

    assertEquals(
        ImmutableSet.of(Strings.repeat("a", 40), Strings.repeat("b", 40)),
        NativeExoHelper.filterLibrariesForAbi(libsDir, allLibs, "armeabi-v7a", ImmutableSet.of())
            .keySet());

    assertEquals(
        ImmutableSet.of(Strings.repeat("d", 40)),
        NativeExoHelper.filterLibrariesForAbi(
                libsDir, allLibs, "armeabi", ImmutableSet.of("libmy1.so", "libmy2.so"))
            .keySet());
  }

  @Test
  public void testParseExopackageInfoMetadata() throws IOException {
    String illegalLine = "no_space_in_this_line_bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    thrown.expectMessage("Illegal line in metadata file: " + illegalLine);

    Path baseDir = Paths.get("basedir");
    AbsPath rootPath = tmpDir.getRoot();
    Path metadataPath = Paths.get("metadata.txt");
    ProjectFilesystemUtils.writeLinesToPath(
        rootPath,
        ImmutableList.of(
            ".some_config",
            ".more_config with_spaces",
            "filename.jar aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "dir/anotherfile.jar bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb"),
        metadataPath);

    assertEquals(
        ImmutableMultimap.of(
            Strings.repeat("a", 40), Paths.get("basedir/filename.jar"),
            Strings.repeat("b", 40), Paths.get("basedir/dir/anotherfile.jar")),
        ExopackageInstaller.parseExopackageInfoMetadata(metadataPath, baseDir, rootPath));

    ProjectFilesystemUtils.writeLinesToPath(
        rootPath,
        ImmutableList.of("filename.jar aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", illegalLine),
        metadataPath);

    ExopackageInstaller.parseExopackageInfoMetadata(metadataPath, baseDir, rootPath);
  }
}
