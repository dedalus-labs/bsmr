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

package com.dedalus.bsmr.android.device;

import com.facebook.infer.annotation.Nullsafe;
import java.util.Optional;

/** Represents information about the device we're targeting. */
@Nullsafe(Nullsafe.Mode.LOCAL)
public class TargetDevice {

  private final Type type;
  private final Optional<String> identifier;

  public enum Type {
    REAL_DEVICE,
    EMULATOR,
    BY_SERIAL
  }

  public TargetDevice(Type type, Optional<String> identifier) {
    this.type = type;
    this.identifier = identifier;
  }

  public boolean isEmulator() {
    return type == Type.EMULATOR;
  }

  public Optional<String> getIdentifier() {
    return identifier;
  }
}
