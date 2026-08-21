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

package com.dedalus.bsmr.jvm.cd;

import com.facebook.infer.annotation.Nullsafe;
import java.io.IOException;

/** A single compilation action created from command line or worker args */
@Nullsafe(Nullsafe.Mode.LOCAL)
public interface JvmCDCommand {
  String WORKING_DIRECTORY_ENV_VAR = "BSMR_SCRATCH_PATH";

  BuildCommandStepsBuilder getBuildCommand();

  String getActionId();

  int getLoggingLevel();

  void postExecute() throws IOException;
}
