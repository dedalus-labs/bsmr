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

import com.dedalus.bsmr.android.exopackage.AndroidDevice;
import com.dedalus.bsmr.android.exopackage.AndroidIntent;
import com.dedalus.bsmr.android.exopackage.PackageInfo;
import com.google.common.collect.ImmutableSortedSet;
import java.io.File;
import java.nio.file.Path;
import java.util.Arrays;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import javax.annotation.Nullable;

/** Basic implementation of AndroidDevice for mocking purposes. */
public class TestAndroidDevice implements AndroidDevice {

  @Override
  public boolean installApkOnDevice(
      File apk,
      boolean installViaSd,
      boolean quiet,
      boolean verifyTempWritable,
      boolean stagedInstallMode,
      @Nullable String userId) {
    throw new UnsupportedOperationException();
  }

  @Override
  public boolean installApexOnDevice(
      File apex,
      boolean quiet,
      boolean restart,
      boolean softRebootAvailable,
      boolean waitForDeviceReady) {
    throw new UnsupportedOperationException();
  }

  @Override
  public boolean prepareForApexInstallation() {
    throw new UnsupportedOperationException();
  }

  @Override
  public void stopPackage(String packageName) throws Exception {
    throw new UnsupportedOperationException();
  }

  @Override
  public Optional<PackageInfo> getPackageInfo(String packageName) throws Exception {
    throw new UnsupportedOperationException();
  }

  @Override
  public void uninstallPackage(String packageName) throws Exception {
    throw new UnsupportedOperationException();
  }

  @Override
  public String getSignature(String packagePath) throws Exception {
    throw new UnsupportedOperationException();
  }

  @Override
  public ImmutableSortedSet<Path> listDirRecursive(Path dirPath) throws Exception {
    throw new UnsupportedOperationException();
  }

  @Override
  public void rmFiles(String dirPath, Iterable<String> filesToDelete) {
    throw new UnsupportedOperationException();
  }

  @Override
  public AutoCloseable createForward() throws Exception {
    throw new UnsupportedOperationException();
  }

  @Override
  public void installFiles(String filesType, Map<Path, Path> installPaths) throws Exception {
    throw new UnsupportedOperationException();
  }

  @Override
  public void mkDirP(String dirpath) throws Exception {
    throw new UnsupportedOperationException();
  }

  @Override
  public String getProperty(String name) throws Exception {
    throw new UnsupportedOperationException();
  }

  @Override
  public List<String> getDeviceAbis() throws Exception {
    throw new UnsupportedOperationException();
  }

  @Override
  public void killProcess(String processName) throws Exception {
    throw new UnsupportedOperationException();
  }

  @Override
  public String getSerialNumber() {
    throw new UnsupportedOperationException();
  }

  @Override
  public String getWindowManagerProperty(String propertyName) throws Exception {
    throw new UnsupportedOperationException();
  }

  @Override
  public boolean isEmulator() {
    throw new UnsupportedOperationException();
  }

  @Override
  public boolean isOnline() {
    throw new UnsupportedOperationException();
  }

  @Override
  public boolean installBuildUuidFile(Path dataRoot, String packageName, String buildUuid)
      throws Exception {
    throw new UnsupportedOperationException();
  }

  @Override
  public String deviceStartIntent(AndroidIntent intent) throws Exception {
    throw new UnsupportedOperationException();
  }

  @Override
  public boolean uninstallApkFromDevice(String packageName, boolean keepData) throws Exception {
    throw new UnsupportedOperationException();
  }

  @Override
  public String getInstallerMethodName() {
    throw new UnsupportedOperationException();
  }

  @Override
  public List<String> getDiskSpace() {
    return Arrays.asList("_", "_", "_");
  }

  @Override
  public void fixRootDir(String rootDir) {}

  @Override
  public boolean setDebugAppPackageName(String packageName) throws Exception {
    throw new UnsupportedOperationException();
  }

  @Override
  public void enableAppLinks(String packageName) throws Exception {
    throw new UnsupportedOperationException();
  }
}
