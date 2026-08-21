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

package com.dedalus.bsmr.cli.bootstrapper;

import static java.util.Objects.requireNonNull;

import java.io.File;
import java.net.MalformedURLException;
import java.net.URL;
import java.net.URLClassLoader;
import java.nio.file.Paths;
import java.util.function.Function;
import java.util.stream.Stream;

/** Creates a ClassLoader from {@code System.getenv()}. classpath entries. */
public class ClassLoaderFactory {

  /** Expose a provider to facilitate mock tests. */
  @FunctionalInterface
  public interface ClassPathProvider extends Function<String, String> {}

  static final String BSMR_CLASSPATH = "BSMR_CLASSPATH";
  static final String EXTRA_BSMR_CLASSPATH = "EXTRA_BSMR_CLASSPATH";

  private final ClassPathProvider classPathProvider;

  @SuppressWarnings("PMD.BlacklistedSystemGetenv")
  static ClassLoaderFactory withEnv() {
    return new ClassLoaderFactory(System.getenv()::get);
  }

  ClassLoaderFactory(ClassPathProvider classPathProvider) {
    this.classPathProvider = requireNonNull(classPathProvider);
  }

  /**
   * Create a new ClassLoader that concats {@value BSMR_CLASSPATH} and {@value EXTRA_BSMR_CLASSPATH}
   *
   * @return ClassLoader instance to env classpath.
   */
  public ClassLoader create() {
    // BSMR_CLASSPATH is not set by a user, no need to use EnvVariablesProvider.
    String classPath = classPathProvider.apply(BSMR_CLASSPATH);
    String extraClassPath = classPathProvider.apply(EXTRA_BSMR_CLASSPATH);

    if (classPath == null) {
      throw new RuntimeException(BSMR_CLASSPATH + " not set");
    }

    URL[] urls =
        Stream.of(classPath, extraClassPath)
            .flatMap(this::splitPaths)
            .filter(this::nonBlank)
            .map(this::toUrl)
            .toArray(URL[]::new);

    return new URLClassLoader(urls);
  }

  private Stream<String> splitPaths(String paths) {
    if (paths == null) {
      return Stream.empty();
    }
    return Stream.of(paths.split(File.pathSeparator));
  }

  private boolean nonBlank(String path) {
    return !path.isBlank();
  }

  private URL toUrl(String path) {
    try {
      return Paths.get(path).toUri().toURL();
    } catch (MalformedURLException e) {
      throw new RuntimeException(e);
    }
  }
}
