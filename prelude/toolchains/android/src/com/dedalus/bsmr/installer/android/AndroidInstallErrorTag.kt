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

package com.dedalus.bsmr.installer.android

import com.dedalus.bsmr.installer.InstallErrorCategory
import com.dedalus.bsmr.installer.InstallErrorCategory.ENVIRONMENT
import com.dedalus.bsmr.installer.InstallErrorCategory.INFRA
import com.dedalus.bsmr.installer.InstallErrorCategory.USER
import com.dedalus.bsmr.installer.InstallErrorTag

enum class AndroidInstallErrorTag(val category: InstallErrorCategory) : InstallErrorTag {
  ERROR_MATERIALIZING_ARTIFACT(INFRA),
  OTHER_INFRA(INFRA),
  ADB_PROTOCOL_VERSION_MISMATCH(ENVIRONMENT),
  ADB_NOT_FOUND(ENVIRONMENT),
  FAILED_TO_CONNECT_TO_ADB(ENVIRONMENT),
  TEMP_FOLDER_NOT_WRITABLE(USER),
  ADB_COMMAND_REJECTED(USER),
  ADB_COMMAND_FAILED(USER),
  ADB_CONNECTION_RESET_BY_PEER(USER),
  ADB_CONNECTION_TIMED_OUT(USER),
  NO_ATTACHED_DEVICE(USER),
  MULTIPLE_DEVICES_MATCH_FILTER(USER),
  DEVICE_NOT_FOUND(USER),
  DEVICE_OFFLINE(USER),
  NO_SPACE_LEFT_ON_DEVICE(USER),
  REQUIRE_NEWER_SDK(USER),
  INCOMPATIBLE_NATIVE_LIB(USER),
  INCOMPATIBLE_ABI(USER),
  UPDATE_INCOMPATIBLE(USER),
  MISSING_SHARED_LIBRARY(USER),
  VERIFICATION_FAILED(USER),
  INVALID_APK(USER),
  INSTALL_CANCELLED_BY_USER(USER),
  UNKNOWN_DEVICE_ABI(USER),
  MANUAL_REBOOT_REQUIRED(USER);

  override fun getErrorCategory(): InstallErrorCategory {
    return category
  }

  override fun getName(): String {
    return name
  }
}
