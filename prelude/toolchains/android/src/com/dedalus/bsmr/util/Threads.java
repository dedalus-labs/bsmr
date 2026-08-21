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

import com.dedalus.bsmr.core.util.log.Logger;
import com.google.common.base.Throwables;

public class Threads {

  private static final Logger LOG = Logger.get(Threads.class);

  /** Utility class: do not instantiate. */
  private Threads() {}

  public static Thread namedThread(String name, Runnable runnable) {
    Thread newThread = new Thread(runnable);
    newThread.setName(name);
    return newThread;
  }

  public static void interruptCurrentThread() {
    LOG.warn(
        "Current thread interrupted at this location: "
            + Throwables.getStackTraceAsString(new Throwable()));
    Thread.currentThread().interrupt();
  }

  public static void interruptThread(Thread thread) {
    LOG.warn(
        "Thread interrupted at this location: "
            + Throwables.getStackTraceAsString(new Throwable()));
    thread.interrupt();
  }
}
