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

package com.dedalus.bsmr.util;

import java.nio.file.Path;
import java.nio.file.Paths;

public class BsmrConstant {

  public static final String DEFAULT_OUTPUT_DIR_NAME = "bsmr-out";
  private static final Path DEFAULT_OUTPUT_PATH =
      Paths.get(System.getProperty("bsmr.base_output_dir", DEFAULT_OUTPUT_DIR_NAME));

  private BsmrConstant() {}

  /** The relative path to the directory where Bsmr will generate its files. */
  public static Path getOutputputPath() {
    return DEFAULT_OUTPUT_PATH;
  }
}
