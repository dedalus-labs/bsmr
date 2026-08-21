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

import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertTrue;

import com.dedalus.bsmr.core.filesystems.AbsPath;
import com.dedalus.bsmr.core.filesystems.RelPath;
import com.dedalus.bsmr.testutil.TemporaryPaths;
import com.dedalus.bsmr.util.ClassLoaderCache;
import com.google.common.collect.ImmutableList;
import com.google.common.collect.ImmutableMap;
import com.google.common.collect.ImmutableSortedSet;
import org.junit.Rule;
import org.junit.Test;

public class PluginFactoryTest {

  @Rule public TemporaryPaths tmp = new TemporaryPaths();

  @Test
  public void testPluginClassloadersNotReusedIfAnyMarkedUnsafe() {
    assertFalse(isPluginClassLoaderReused(false)); // safe processors
  }

  @Test
  public void testPluginClassloadersReusedIfAllMarkedSafe() {
    assertTrue(isPluginClassLoaderReused(true)); // safe processors
  }

  private boolean isPluginClassLoaderReused(boolean canReuseClasspath) {
    RelPath controlClasspath = RelPath.get("some/path/to.jar");
    RelPath variableClasspath = RelPath.get("some/path/to_other.jar");

    ClassLoader baseClassLoader = ClassLoader.getPlatformClassLoader();
    ClassLoaderCache classLoaderCache = new ClassLoaderCache();

    AbsPath rootPath = tmp.getRoot();

    ResolvedJavacPluginProperties controlPluginGroup =
        new ResolvedJavacPluginProperties(
            true /* canReuseClassLoader */, // control can always reuse
            false /* doesNotAffectAbi */,
            false /* supportsAbiGenerationFromSource */,
            false /* runsOnJavaOnly */,
            ImmutableSortedSet.of("controlPlugin"),
            ImmutableList.of(controlClasspath),
            ImmutableMap.of() /* pathParams */,
            ImmutableList.of() /* arguments */);

    ResolvedJavacPluginProperties variablePluginGroup =
        new ResolvedJavacPluginProperties(
            canReuseClasspath,
            false /* doesNotAffectAbi */,
            false /* supportsAbiGenerationFromSource */,
            false /* runsOnJavaOnly */,
            ImmutableSortedSet.of("variablePlugin"),
            ImmutableList.of(variableClasspath),
            ImmutableMap.of() /* pathParams */,
            ImmutableList.of() /* arguments */);

    try (PluginFactory factory1 = new PluginFactory(baseClassLoader, classLoaderCache);
        PluginFactory factory2 = new PluginFactory(baseClassLoader, classLoaderCache)) {

      JavacPluginParams pluginParams =
          new JavacPluginParams(
              ImmutableList.of(controlPluginGroup, variablePluginGroup), ImmutableSortedSet.of());
      ClassLoader classLoader1 = factory1.getClassLoaderForProcessorGroups(pluginParams, rootPath);
      ClassLoader classLoader2 = factory2.getClassLoaderForProcessorGroups(pluginParams, rootPath);
      return classLoader1 == classLoader2;
    } catch (Exception e) {
      throw new AssertionError(e);
    }
  }
}
