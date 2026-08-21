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

import static org.junit.Assert.assertEquals;

import com.dedalus.bsmr.jvm.java.fatjar.WindowsCreateProcessEscape;
import org.junit.Test;

public class WindowsCreateProcessEscapeTest {
  @Test
  public void testCases() {
    // An array of of input strings and the expected output.
    String[][] tests = {
      {
        "C:\\Windows\\", "C:\\Windows\\",
      },
      {
        "", "\"\"",
      },
      {
        " ", "\" \"",
      },
      {
        "\t", "\"\t\"",
      },
      {
        "\\", "\\",
      },
      {
        "\\\\", "\\\\",
      },
      {
        " \\", "\" \\\\\"",
      },
      {
        "\t\\", "\"\t\\\\\"",
      },
      {
        "\\\"", "\"\\\\\\\"\"",
      },
      {
        "\\a\\\"", "\"\\a\\\\\\\"\"",
      },
      {
        "\\\"a\\\"", "\"\\\\\\\"a\\\\\\\"\"",
      },
      {
        "\\\"\\\"", "\"\\\\\\\"\\\\\\\"\"",
      },
    };

    for (String[] test : tests) {
      assertEquals(2, test.length);
      assertEquals(test[1], WindowsCreateProcessEscape.quote(test[0]));
    }
  }
}
