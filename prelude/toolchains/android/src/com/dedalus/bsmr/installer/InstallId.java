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

package com.dedalus.bsmr.installer;

import com.google.common.base.Preconditions;
import com.google.common.base.Strings;
import java.util.Objects;

/** Install id. Typically represents fully qualified name of the build target. */
public class InstallId {

  private final String value;

  private InstallId(final String value) {
    this.value = value;
  }

  public String getValue() {
    return value;
  }

  @Override
  public int hashCode() {
    return Objects.hash(value);
  }

  @Override
  public boolean equals(Object obj) {
    if (this == obj) {
      return true;
    }
    if (obj == null || getClass() != obj.getClass()) {
      return false;
    }
    return value.equals(((InstallId) obj).value);
  }

  public static InstallId of(String value) {
    Preconditions.checkState(!Strings.isNullOrEmpty(value), "install id has to be not empty");
    return new InstallId(value);
  }
}
